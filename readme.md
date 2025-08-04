# **cargo-check-i18n**

A Cargo plugin that intercepts `cargo check` output, translates diagnostic messages into your target language using LLM, and displays them inline.

---

## Features

- **Real‑time translation** of `cargo check` output.
- Configurable target language (e.g. `zh-CN`, `ja-JP`).
- Caching of translations to accelerate repeat runs.
- Simple **TOML** configuration in `~/.config/cargo-check-i18n/config.toml`.

---

## Installation

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
   Configuration Instructions:

   3.1. Open ~/.config/cargo-check-i18n/config.toml (a sample will be generated automatically upon first run).

   3.2. Fill in your API information, for example:

   ```toml
   version = "1.0"
   language = "zh-CN"
   api_url = "https://api.openai.com/v1/chat/completions"         # LLM API endpoint
   api_key = "sk-xxxx"                                            # API key
   rate_limit = 8
   model = "gpt-4o-mini"                                          # Model name
   temperature = 0.2                                              # Temperature parameter

   # Request body template. Supports {{model}}, {{prompt}}, and {{temperature}} variables.
   request_body_template = """
   {
      "model": "{{model}}",
      "messages": [{"role": "user", "content": "{{prompt}}"}],
      "temperature": {{temperature}}
   }
   """
   # JSON path to the response, using dot notation, e.g., choices.0.message.content
   response_path = "choices.0.message.content"
   ```
   3.3 If you are using an API from another provider, simply adjust `request_body_template` and `response_path` according to their documentation.
   For example, if the API returns {"result": "translated content"}, then `response_path` can be set to "result".

   3.4. After saving the configuration, run `cargo-check-i18n` to automatically call the specified API and display the translation result.

4. Add `/.cargo/bin` to your system `PATH` so you can run installed tools globally:
   for Linux or macOS (bash / zsh)
   ```
   export PATH="$HOME/.cargo/bin:$PATH"
   source ~/.bashrc   # or ~/.zshrc
   ```
   for Windows Powershell
   ```  
   [Environment]::SetEnvironmentVariable("Path","$env:USERPROFILE\.cargo\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"),"User")
   Start-Process powershell -ArgumentList '-NoExit' -Wait
   ```

---

## Usage

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

## Contributing

Pull requests, issues, and suggestions are welcome!

---

