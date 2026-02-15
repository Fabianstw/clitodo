# Contributing

Thanks for improving todo.

## Development commands

Format the code:

```bash
cargo fmt
```

Run tests:

```bash
cargo test
```

Optional linting (if you use clippy locally):

```bash
cargo clippy
```

## CLI guide

- LaTeX source: [docs/cli-guide.tex](docs/cli-guide.tex)
- PDF (generated): [docs/cli-guide.pdf](docs/cli-guide.pdf)

To build the PDF locally (if you have LaTeX installed):

```bash
pdflatex -output-directory docs docs/cli-guide.tex
```
