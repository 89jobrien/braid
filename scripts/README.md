# TSI - Test Session Interface

TSI is a global command-line interface for the Braid project that provides convenient access to all Braid functionality from anywhere on your system.

## Installation

### Quick Install
```bash
# From the braid project root
./scripts/install-tsi.sh
```

### Symlink Install (for development)
```bash
# Creates a symlink instead of copying (useful during development)
./scripts/install-tsi.sh --symlink
```

### Manual Install
```bash
# Copy the script to your local bin directory
cp scripts/tsi ~/.local/bin/tsi
chmod +x ~/.local/bin/tsi

# Make sure ~/.local/bin is in your PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

## Usage

### Basic Commands

```bash
# Run a Braid session
tsi run "Hello, world!"

# Check environment health
tsi doctor

# Build the workspace
tsi build

# Run tests
tsi test

# Start MCP server
tsi mcp

# Show help
tsi help
```

### Advanced Usage

```bash
# Use specific provider and model
tsi run --provider ollama --model llama3.2 "Write a Python function"

# Use OpenAI
tsi run --provider openai --model gpt-4o "Explain this code"

# Run specific tests
tsi test braid-core

# Verbose output
tsi --verbose run "Debug this issue"
```

## Features

- **Smart Provider Detection**: Automatically detects available providers (OpenAI, Ollama)
- **Environment Integration**: Uses `uv run` for consistent Python environment management
- **Colored Output**: Clear, colored logging for better visibility
- **Error Handling**: Robust error handling with helpful messages
- **Development Workflow**: Integrates building, testing, and running in one command

## Configuration

TSI reads configuration from:
- Environment variables (e.g., `OPENAI_API_KEY`)
- Command line arguments
- Project defaults

### Environment Variables

- `OPENAI_API_KEY` - OpenAI API key for GPT models
- `OLLAMA_HOST` - Ollama server host (default: localhost:11434)

## Commands Reference

| Command | Description | Examples |
|---------|-------------|----------|
| `run` | Execute a Braid session | `tsi run "Hello"` |
| `doctor` | Check environment health | `tsi doctor` |
| `build` | Build the workspace | `tsi build` |
| `test` | Run tests | `tsi test`, `tsi test braid-core` |
| `mcp` | Start MCP server | `tsi mcp` |
| `help` | Show help information | `tsi help` |

### Options

| Option | Description | Example |
|--------|-------------|---------|
| `--provider <name>` | Specify provider (ollama, openai) | `--provider ollama` |
| `--model <name>` | Specify model name | `--model gpt-4o` |
| `--verbose` | Enable verbose output | `--verbose` |
| `--help, -h` | Show help | `--help` |

## Integration

TSI is designed to work seamlessly with:
- **uv**: Python package management and execution
- **Cargo**: Rust build system and package manager  
- **Git**: Version control workflows
- **VS Code**: Development environment integration

## Troubleshooting

### Command not found
```bash
# Check if ~/.local/bin is in PATH
echo $PATH | grep -q "$HOME/.local/bin" && echo "✓ PATH OK" || echo "✗ Add ~/.local/bin to PATH"

# Add to PATH if missing
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Build failures
```bash
# Check environment
tsi doctor

# Clean and rebuild
cd ~/dev/braid
uv run cargo clean
tsi build
```

### Provider issues
```bash
# Check OpenAI setup
echo $OPENAI_API_KEY | cut -c1-10  # Should show key prefix

# Check Ollama
curl -sf http://localhost:11434/api/tags
```

## Development

To modify TSI:

1. Edit `scripts/tsi`
2. If installed with `--symlink`, changes are immediately available
3. If copied, re-run `./scripts/install-tsi.sh`
4. Test with `tsi --help`

## Examples

```bash
# Quick session
tsi run "What's the weather like?"

# Development workflow
tsi build
tsi test
tsi run "Review my latest code changes"

# Use local Ollama
tsi run --provider ollama --model codellama "Explain this Rust function"

# Health check before deployment
tsi doctor
```