//! Offered load: requests that arrive at a rate of their own, whatever the engine has answered.
//!
//! A closed loop sends the next request when the last one returns, so an engine that slows
//! down is offered less work and its queue never shows. Here the arrivals are fixed in advance.
//! An engine that serves them keeps its latency flat. An engine that cannot lets the latency
//! of each request exceed the one before it.
//!
//! The search raises the rate until the latency rises over a step, then narrows the interval
//! between the last rate served and the first rate that was not.

use std::future::Future;

/// How the arrivals of a step are spaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// One request every `1 / rate` seconds.
    Constant,
    /// Gaps drawn from an exponential law of mean `1 / rate`: a Poisson process.
    Random,
}

impl Profile {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "constant" => Some(Self::Constant),
            "random" | "poisson" => Some(Self::Random),
            _ => None,
        }
    }
}

/// A 64-bit xorshift generator: the arrivals of a step are the same on every run.
struct Rng(u64);

impl Rng {
    fn next_unit(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        // 53 random bits in (0, 1].
        ((x >> 11) as f64 + 1.0) / (1u64 << 53) as f64
    }
}

/// The arrival time of each of `count` requests, in seconds from the start of the step, at a
/// mean of `rate` requests per second.
pub fn arrivals(profile: Profile, rate: f64, count: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng(seed | 1);
    let mut at = 0.0;
    (0..count)
        .map(|_| {
            let t = at;
            at += match profile {
                Profile::Constant => 1.0 / rate,
                Profile::Random => -rng.next_unit().ln() / rate,
            };
            t
        })
        .collect()
}

/// One-sided 95% critical value of Student's t for `df` degrees of freedom.
fn t_critical(df: usize) -> f64 {
    const TABLE: [f64; 30] = [
        6.314, 2.920, 2.353, 2.132, 2.015, 1.943, 1.895, 1.860, 1.833, 1.812, 1.796, 1.782, 1.771,
        1.761, 1.753, 1.746, 1.740, 1.734, 1.729, 1.725, 1.721, 1.717, 1.714, 1.711, 1.708, 1.706,
        1.703, 1.701, 1.699, 1.697,
    ];
    match df {
        0 => f64::INFINITY,
        1..=30 => TABLE[df - 1],
        _ => 1.645,
    }
}

/// Whether the latencies, in arrival order, rise: the slope of their least-squares line is
/// above zero at the one-sided 95% level. A step with fewer than 3 latencies says nothing.
pub fn queue_grows(latencies: &[f64]) -> bool {
    let n = latencies.len();
    if n < 3 {
        return false;
    }
    let nf = n as f64;
    let mean_x = (nf - 1.0) / 2.0;
    let mean_y = latencies.iter().sum::<f64>() / nf;
    let sxx: f64 = (0..n).map(|i| (i as f64 - mean_x).powi(2)).sum();
    let sxy: f64 = latencies
        .iter()
        .enumerate()
        .map(|(i, y)| (i as f64 - mean_x) * (y - mean_y))
        .sum();
    let slope = sxy / sxx;
    if slope <= 0.0 {
        return false;
    }
    let residual: f64 = latencies
        .iter()
        .enumerate()
        .map(|(i, y)| (y - mean_y - slope * (i as f64 - mean_x)).powi(2))
        .sum();
    let stderr = (residual / (nf - 2.0) / sxx).sqrt();
    if stderr == 0.0 {
        return true;
    }
    slope / stderr > t_critical(n - 2)
}

/// What one step measured.
#[derive(Debug, Clone)]
pub struct Step {
    /// Offered rate, requests per second.
    pub rate: f64,
    /// Latency of each request that was answered, milliseconds, in arrival order.
    pub latencies_ms: Vec<f64>,
    /// Requests that failed.
    pub failed: usize,
    pub grows: bool,
}

/// What the search found.
#[derive(Debug, Clone)]
pub struct Search {
    pub steps: Vec<Step>,
    /// The highest rate a step served without its latency rising; `None` when no step did.
    pub sustained: Option<f64>,
}

/// Find the rate at which the latency starts to rise. `run` offers `count` requests at the
/// rate it is given and returns the latency of each, `None` for a request that failed.
///
/// The search starts at `start`, doubles while the engine keeps up, halves while it does not,
/// then bisects until the interval is narrower than the relative precision `count` requests
/// can resolve.
pub async fn find_saturation<F, Fut>(start: f64, count: usize, mut run: F) -> Search
where
    F: FnMut(f64) -> Fut,
    Fut: Future<Output = Vec<Option<f64>>>,
{
    let precision = 1.0 / (count as f64).sqrt();
    let mut steps: Vec<Step> = Vec::new();
    let step = |rate: f64, answers: Vec<Option<f64>>| {
        let latencies_ms: Vec<f64> = answers.iter().flatten().copied().collect();
        let failed = answers.len() - latencies_ms.len();
        let grows = queue_grows(&latencies_ms);
        Step {
            rate,
            latencies_ms,
            failed,
            grows,
        }
    };
    // An engine that answers nothing has no rate to find.
    let broken = |s: &Step| s.latencies_ms.is_empty();

    let (mut served, mut refused): (Option<f64>, Option<f64>) = (None, None);
    let mut rate = start;
    for _ in 0..64 {
        let s = step(rate, run(rate).await);
        let (grows, empty) = (s.grows, broken(&s));
        steps.push(s);
        if empty {
            return Search {
                steps,
                sustained: served,
            };
        }
        if grows {
            refused = Some(rate);
            if served.is_some() {
                break;
            }
            rate /= 2.0;
        } else {
            served = Some(rate);
            if refused.is_some() {
                break;
            }
            rate *= 2.0;
        }
    }
    while let (Some(lo), Some(hi)) = (served, refused) {
        if hi / lo - 1.0 < precision {
            break;
        }
        let mid = (lo * hi).sqrt();
        let s = step(mid, run(mid).await);
        let (grows, empty) = (s.grows, broken(&s));
        steps.push(s);
        if empty {
            break;
        }
        if grows {
            refused = Some(mid);
        } else {
            served = Some(mid);
        }
    }
    Search {
        steps,
        sustained: served,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_arrivals_are_evenly_spaced() {
        let a = arrivals(Profile::Constant, 4.0, 5, 1);
        assert_eq!(a, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn random_arrivals_keep_the_rate_and_repeat() {
        let a = arrivals(Profile::Random, 10.0, 4000, 7);
        assert_eq!(a, arrivals(Profile::Random, 10.0, 4000, 7));
        let mean_gap = a.last().unwrap() / (a.len() - 1) as f64;
        assert!((mean_gap - 0.1).abs() < 0.01, "mean gap {mean_gap}");
        assert!(a.windows(2).all(|w| w[1] > w[0]));
    }

    #[test]
    fn a_rising_latency_is_a_growing_queue() {
        let rising: Vec<f64> = (0..20).map(|i| 100.0 + 10.0 * i as f64).collect();
        assert!(queue_grows(&rising));
    }

    #[test]
    fn a_flat_or_noisy_latency_is_not() {
        let flat = vec![100.0; 20];
        assert!(!queue_grows(&flat));
        let noisy: Vec<f64> = (0..20)
            .map(|i| 100.0 + if i % 2 == 0 { 5.0 } else { -5.0 })
            .collect();
        assert!(!queue_grows(&noisy));
        let falling: Vec<f64> = (0..20).map(|i| 300.0 - 10.0 * i as f64).collect();
        assert!(!queue_grows(&falling));
        assert!(!queue_grows(&[1.0, 2.0]));
    }

    /// An engine that serves 8 requests per second: below that its latency is flat, above it
    /// each request waits for the ones before.
    async fn engine(rate: f64, count: usize) -> Vec<Option<f64>> {
        let service = 1.0 / 8.0;
        let mut free_at = 0.0f64;
        arrivals(Profile::Constant, rate, count, 1)
            .into_iter()
            .map(|at| {
                let start = free_at.max(at);
                free_at = start + service;
                Some((free_at - at) * 1000.0)
            })
            .collect()
    }

    #[tokio::test]
    async fn the_search_lands_just_under_the_rate_an_engine_serves() {
        let found = find_saturation(1.0, 40, |rate| engine(rate, 40)).await;
        let sustained = found.sustained.expect("a rate was served");
        assert!(
            sustained <= 8.0 && sustained > 8.0 / 1.2,
            "sustained {sustained}"
        );
    }

    #[tokio::test]
    async fn a_search_starting_above_the_rate_halves_down_to_it() {
        let found = find_saturation(64.0, 40, |rate| engine(rate, 40)).await;
        let sustained = found.sustained.expect("a rate was served");
        assert!(sustained <= 8.0 && sustained > 8.0 / 1.2);
    }

    #[tokio::test]
    async fn an_engine_that_answers_nothing_has_no_rate() {
        let found = find_saturation(1.0, 10, |_| async { vec![None; 10] }).await;
        assert_eq!(found.sustained, None);
        assert_eq!(found.steps.len(), 1);
    }
}
