**# cargo-check-i18n**

A Cargo plugin that intercepts `cargo check` output, translates diagnostic messages into your target language using OpenAI, and displays them inline.

---

**## Features**

- **Real‑time translation** of `cargo check` output.
- Configurable target language (e.g. `zh-CN`, `ja-JP`).
- Caching of translations to accelerate repeat runs.
- Simple **TOML** configuration in `~/.config/cargo-check-i18n/config.toml`.

---

**## Installation**

1. Clone the repository:

   ```bash
   git clone https://github.com/yuuenn/cargo-check-i18n.git
   cd cargo-check-i18n
   ```

2. Build and install locally:

   ```bash
   cargo install --path .
   ```

3. Ensure `~/.config/cargo-check-i18n/config.toml` exists and contains your LLM API key:

   ```toml
   version = "1.0"
   language = "zh-CN"
   api_url = "https://api.openai.com/v1/chat/completions"
   api_key = "YOUR_LLM_API_KEY"
   model = "gpt-4o-mini"
   temperature = 0.2
   ```

4. (Optional) Add `$HOME/.cargo/bin` to your system `PATH` so you can run installed tools globally:

   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   ```

---

**## Usage**

### 🔧 Local Development and Testing (for contributors)

To test this tool locally while developing:

1. Create a temporary test Rust project inside this repository:

   ```bash
   mkdir test
   cd test
   cargo init
   ```

2. Inside `test/src/main.rs`, write some code with intentional mistakes (e.g., undefined variables).

3. Run the plugin directly using:

   ```bash
   cargo run -- check
   ```

> Note: This will compile and run the current crate (i.e. `cargo-check-i18n`) and pass `check` as an argument to your program.

---

### 🚀 Installed Usage (for end users)

Once installed, you can use the tool globally on **any Rust project**:

```bash
cargo i18n check
```

This command intercepts `cargo check` in the current Rust project and translates compiler diagnostics based on your configuration.

Alternatively, you can run:

```bash
cargo-check-i18n check
```

Both forms are equivalent as long as the binary is in your PATH.

---

**## Contributing**

Pull requests, issues, and suggestions are welcome!

---

