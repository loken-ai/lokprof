# Prompts

The built-in prompts that `--prompts` names.
Every target receives the same bytes.

<!-- site: widget table id=lokprof-prompts-builtin -->
| Name | Bytes | Words | Text |
|---|---|---|---|
| `short` | 58 | 13 | The opening of a story, to continue |
| `medium` | 236 | 34 | A paragraph on the history of computing, to continue |
| `long` | 939 | 139 | A request to review a Rust function, with the function |
| `2.5k` | 11053 | 1734 | An article on processor architecture, then questions on it |
| `ctx2000` | 8376 | 1507 | Random words, to fill the context |
| `ctx8000` | 32818 | 6007 | Random words, to fill the context |
| `vision_short` | 20 | 3 | A request to describe the image |
| `vision_medium` | 174 | 28 | A request to describe the image in about 3 sentences |
| `vision_long` | 465 | 74 | A request to describe the image in a paragraph of 6 sentences at least |
<!-- site: end -->

`2.5K` is another name for `2.5k`.  
The default is `short,medium,long`.

The text prompts are written as texts to continue, and reach every engine with no chat template.  
`2.5k`, `ctx2000` and `ctx8000` fill the context while asking for the same answer length as the others.
Their cells measure how the decode rate falls as the context grows.  
The vision prompts are instructions, and need `--image`.

The record stores the name of the prompt in `prompt` and its length in bytes in `prompt_chars`.
`--prompt TEXT` replaces the list with 1 prompt named `custom`.
