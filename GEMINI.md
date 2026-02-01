# Project Context and Guidelines

## Development Instructions

1.  **Context Management:** To keep the Gemini context as lean as possible, avoid providing full file contents if only small changes are needed. Prefer using `grep` or `read_file` with specific line ranges to investigate.

## Testing Strategy

- **Always** check for existing unit tests when modifying Rust code.
- **Add** unit tests for new logic to ensure correctness.
- Follow the project's mocking approach: each module should provide mocks if it has external dependencies.
- In tests, prefer `expect("message")` over `unwrap()` to provide context on failure.
- When testing for specific error messages, use `unwrap_err()` and assert on the error.
- For complex assertions on `Result` types, `match` statements are a good option.

## Quality Checks

Run these commands to ensure code quality (matching CI):

```bash
# Check formatting
cargo fmt --check

# Run linter
cargo clippy --workspace --all-targets --all-features --no-deps
```
