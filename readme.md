# cargo-check-i18n

A Cargo plugin that intercepts `cargo check` output, translates diagnostic messages into your target language using OpenAI, and displays them inline.

## Features

- **Real‑time translation** of `cargo check` output.
- Configurable target language (e.g. `zh-CN`, `ja-JP`).
- Caching of translations to accelerate repeat runs.
- Simple **TOML** configuration in `~/.config/cargo-check-i18n/config.toml`.

## Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/yuuenn/cargo-check-i18n.git
   cd cargo-check-i18n
   ```
2. Build and install:
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

## Usage

Run translations in place of `cargo check`:

```bash
cargo-check-i18n check
```

Or if installed as `cargo-check-i18n`, Cargo will detect it:

```bash
cargo i18n
```

## Contributing

Pull requests, issues, and suggestions are welcome!

---

