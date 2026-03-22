# Contributing to Strung

We're excited you're interested in improving **Strung**! Whether it's fixing bugs, adding new filters, or optimizing the engine, all contributions are welcome.

## 🚀 Getting Started

1.  **Fork** the repository.
2.  **Clone** your fork.
3.  **Build** the project: `cargo build`.
4.  **Run tests**: `cargo test`.

## 🛠 Adding New Filters

To add a new junk filter:
1.  Open `src/main.rs`.
2.  Add your logic to the `is_not_junk` function or create a new check in `get_significance_score`.
3.  Add a sample test case in `tests/samples/` to verify your improvement.

## 🐛 Reporting Bugs

Please use GitHub Issues to report bugs. Include:
- A description of the issue.
- The command you ran.
- A sample of the input file (if possible).
- Expected vs. actual results.

## 📜 Pull Request Process

1.  Ensure your code follows the existing style.
2.  Update the documentation if you add new flags or features.
3.  Ensure all tests pass.
4.  Submit your PR and wait for review!

---

Thank you for making Strung better! 🧵
