# nutrition-rs

A plain-text nutrition tracking tool built around the **Nutrition language** — a simple, human-readable file format for defining ingredients, recipes, exercises, and daily logs.

`nutrition-rs` ships:
- A **Rust CLI** (`nutrition`) for validating, querying, and reporting on `.nutrition` files
- A **VS Code extension** with syntax highlighting, navigation, and formatting support

> [!WARNING]
> **Early development — expect breaking changes.** The Nutrition language spec and the CLI API are both unstable. Syntax, keywords, command names, flags, and output formats may change in any release without notice. The project is not yet suitable for production use.

---

## Table of Contents

- [The Nutrition Language](#the-nutrition-language)
  - [File Format](#file-format)
  - [Imports](#imports)
  - [Comments](#comments)
  - [Quantities](#quantities)
  - [Properties](#properties)
  - [Declarations](#declarations)
    - [@ingredient](#ingredient)
    - [@recipe](#recipe)
    - [@exercise](#exercise)
    - [@day](#day)
- [CLI Usage](#cli-usage)
  - [Installation](#installation)
  - [Global Options](#global-options)
  - [validate](#validate)
  - [query](#query)
  - [report](#report)
  - [generate](#generate)
  - [serve](#serve)
- [VS Code Extension](#vs-code-extension)
  - [Features](#features)
  - [Installation](#installation-1)
  - [Building from Source](#building-from-source)
- [GitHub Syntax Highlighting](#github-syntax-highlighting)
- [Examples](#examples)

---

## The Nutrition Language

Nutrition files use the `.nutrition` extension and contain a series of **declarations** that describe ingredients, recipes, exercises, and daily food logs.

### File Format

A `.nutrition` file is a plain-text document consisting of top-level declarations separated by whitespace. Order matters only for readability — the parser collects all declarations before computing reports.

```nutrition
// This is a nutrition file

@ingredient(100g) "oats" {
  calories: 389
  protein: 17g
  carbohydrates: 66g
  fat: 7g
  fiber: 10g
}

@recipe(2) "oatmeal" {
  "oats"(80g)
}

@day "2026-01-01" {
  @ate "oatmeal"(1)
}
```

### Imports

Use `!import` to include declarations from another `.nutrition` file.

```nutrition
!import "./shared_ingredients.nutrition"
```

- The import path must be a quoted string.
- You can add an inline comment after the import line.
- Imports are parsed as first-class top-level items, so files that begin with `!import` parse correctly.
- CLI commands load imported files recursively (with cycle detection), so queries/reports can resolve declarations across imported files.

### Comments

Line comments begin with `//` and may appear anywhere on a line.

```nutrition
// This is a full-line comment
@ingredient(100g) "sugar" { // inline comment
  calories: 387 // kcal per 100g
}
```

### Quantities

A **quantity** is a numeric value followed by an optional unit, with no required space between them.

```nutrition
100g        // 100 grams
2.5 cups    // 2.5 cups (space before unit is allowed)
30 min      // 30 minutes
1           // dimensionless count
```

When an ingredient defines multiple quantity equivalencies, the parser records all of them so that cross-unit conversions (e.g. grams ↔ cups) work when referencing that ingredient.

### Properties

A **property** is a named nutritional attribute with a quantity value, written as `name: value` inside a block:

```nutrition
calories: 269
protein: 14.5g
fat: 4g
carbohydrates: 45g
fiber: 12.5g
```

Property names are arbitrary identifiers. The unit-resolution system recognises common names (e.g. `calories` defaults to `kcal`, `protein`/`fat`/`carbohydrates`/`fiber` default to `g`) so that unitless values interoperate correctly with explicit-unit values.

---

### Declarations

#### `@ingredient`

Defines a food ingredient with one or more quantity equivalencies, one or more name aliases, and a property block.

**Syntax:**
```nutrition
@ingredient(<quantity>)... "<alias>"... {
  <property>: <quantity>
  ...
}
```

- **Quantities** — one or more `(<quantity>)` groups defining the base serving size and optional alternate measurements (e.g. a weight and a volume). The first quantity is the canonical base.
- **Aliases** — one or more quoted strings. Any alias can be used when referencing the ingredient in a recipe or day log.
- **Alias layout** — aliases may appear on the same line as `@ingredient` or be newline-delimited across multiple lines before the `{` block.
- **Properties** — nutritional facts per the base serving size. The block may be empty (`{}`).

**Examples:**
```nutrition
// Single serving size
@ingredient(100g) "sugar" {
  calories: 387
  carbohydrates: 100g
}

// Multiple serving sizes (100g = 1 cup for this ingredient)
@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" {
  calories: 269
  protein: 14.5g
  fat: 4g
  carbohydrates: 45g
  fiber: 12.5g
}

// Equivalent newline-delimited alias form
@ingredient(100g)(1 cup)
"chickpeas"
"chickpea"
"garbanzo beans" {
  calories: 269
  protein: 14.5g
  fat: 4g
  carbohydrates: 45g
  fiber: 12.5g
}
```

---

#### `@recipe`

Defines a recipe as a combination of ingredients and specifies how many servings (or what total quantity) it yields.

**Syntax:**
```nutrition
@recipe(<yield-quantity>)... "<alias>"... {
  "<ingredient-alias>"(<quantity>)
  ...
}
```

- **Yield quantities** — how much the recipe makes (e.g. `(4)` for 4 servings, `(500g)` for 500 g total). Multiple quantities define alternate yield representations.
- **Aliases** — quoted name(s) for the recipe.
- **Alias layout** — aliases may be inline or newline-delimited before the `{` block.
- **Ingredient list** — each line is `"<alias>"(<quantity>)`, referencing any alias of an `@ingredient` or another `@recipe`. Entries can optionally be comma-separated.

**Examples:**
```nutrition
@recipe(8)(500g) "chickpea stew" {
  "chickpeas"(2 cups)
  "water"(5 cups)
}

@recipe(4) "simple chickpeas" {
  "water"(1 cup)
  "chickpeas"(200g)
}
```

Nutritional values are computed by resolving each ingredient reference, scaling its properties to the requested quantity, and summing across all ingredients.

---

#### `@exercise`

Defines an exercise with a canonical duration/quantity and the calories (or other properties) it burns per that quantity.

**Syntax:**
```nutrition
@exercise(<quantity>)... "<alias>"... {
  <property>: <quantity>
  ...
}
```

**Example:**
```nutrition
@exercise(30 min) "running" {
  calories: 300kcal
}
```

Exercise aliases follow the same rule as ingredients/recipes: one or more quoted aliases may be inline or newline-delimited before the `{` block.

When referenced in a `@day` block via `@exercised`, the quantity is scaled to match the duration logged.

---

#### `@day`

Logs a single day's food intake and exercise. The date must be an ISO-8601 date string (`YYYY-MM-DD`).

**Syntax:**
```nutrition
@day "<YYYY-MM-DD>" {
  @ate "<food-alias>"(<quantity>)
  @exercised "<exercise-alias>"(<quantity>)
  ...
}
```

- **`@ate`** — records eating a food (ingredient or recipe). The quantity is the number of servings or a weight/volume.
- **`@exercised`** — records performing an exercise at a given quantity/duration.

**Example:**
```nutrition
@day "2026-01-06" {
  @ate "simple chickpeas"(3)
  @exercised "running"(30 min)
}
```

The `report` command aggregates all `@ate` entries for a day to produce an **intake** total, all `@exercised` entries to produce an **exercise** total, and subtracts them to give a **net** value.

---

## CLI Usage

### Installation

**From source** (requires [Rust](https://rustup.rs/)):

```bash
git clone https://github.com/jafayer/nutrition-rs.git
cd nutrition-rs
cargo build --release
# Binary is at target/release/nutrition
```

You can add it to your `PATH` or install it directly:

```bash
cargo install --path .
```

### Global Options

| Flag | Env Variable | Description |
|------|-------------|-------------|
| `-f, --file <FILE>` | `NUTRITION_DEFAULT_FILE` | Path to the `.nutrition` file to operate on |

All subcommands that operate on a file require `--file` (or the environment variable).

```bash
export NUTRITION_DEFAULT_FILE=~/my-food-log.nutrition
nutrition report        # uses $NUTRITION_DEFAULT_FILE
```

---

### `validate`

Parse a `.nutrition` file and report whether it is syntactically valid.

```bash
nutrition --file <FILE> validate [--show-tree]
```

| Flag | Description |
|------|-------------|
| `--show-tree` | Print the parsed AST after successful validation |

**Example:**
```bash
nutrition --file diet.nutrition validate
# File 'diet.nutrition' is valid.

nutrition --file diet.nutrition validate --show-tree
# File 'diet.nutrition' is valid.
# Document { items: [ ... ] }
```

---

### `query`

Look up the nutritional information for a named ingredient or recipe, optionally scaled to a specific quantity.

```bash
nutrition --file <FILE> query --name <NAME> [--quantity <QUANTITY>] [--json]
```

| Flag | Description |
|------|-------------|
| `-n, --name <NAME>` | Name or alias of the ingredient or recipe |
| `-q, --quantity <QUANTITY>` | Quantity to scale to, e.g. `200g` or `2 servings` |
| `--json` | Output raw JSON instead of the formatted nutrition label |

**Examples:**
```bash
# Query at base serving size
nutrition --file diet.nutrition query --name "chickpeas"

# Query scaled to 200g
nutrition --file diet.nutrition query --name "chickpeas" --quantity 200g

# Machine-readable JSON output
nutrition --file diet.nutrition query --name "chickpea stew" --json
```

---

### `report`

Compute daily nutrition reports from `@day` blocks. By default, shows today's entry.

```bash
nutrition --file <FILE> report [--start <DATE>] [--end <DATE>] [--ate-only] [--no-aggregate] [--json]
```

| Flag | Description |
|------|-------------|
| `--start <DATE>` | Start date, inclusive (`YYYY-MM-DD` or `today`). Defaults to today. |
| `--end <DATE>` | End date, inclusive (`YYYY-MM-DD` or `today`). Defaults to today. |
| `--ate-only` | Show only intake; exclude exercise and net computation |
| `--no-aggregate` | Show each day individually instead of averaging over the range |
| `--json` | Output raw JSON |

When `--start` and `--end` span multiple days the results are **aggregated** (averaged per day) by default. Use `--no-aggregate` to see each day separately.

**Examples:**
```bash
# Today's report
nutrition --file diet.nutrition report

# A specific day
nutrition --file diet.nutrition report --start 2026-01-06 --end 2026-01-06

# Weekly average
nutrition --file diet.nutrition report --start 2026-01-01 --end 2026-01-07

# All days individually, intake only, as JSON
nutrition --file diet.nutrition report \
  --start 2026-01-01 --end 2026-01-31 \
  --no-aggregate --ate-only --json
```

---

### `generate`

Scaffold new declarations and emit them as formatted Nutrition language text, ready to paste into a `.nutrition` file.

```bash
nutrition generate <SUBCOMMAND>
```

#### `generate ingredient`

```bash
nutrition generate ingredient \
  --quantity <QUANTITY> \
  --alias <ALIAS> \
  [--property <NAME:VALUE>] \
  [--ai]
```

| Flag | Description |
|------|-------------|
| `-q, --quantity <QUANTITY>` | Serving size(s) — repeat for multiple equivalencies |
| `-a, --alias <ALIAS>` | Name(s) — repeat for multiple aliases |
| `-p, --property <NAME:VALUE>` | Property in `name:value` format — repeat for multiple |
| `--ai` | Use OpenAI to auto-fill nutritional properties |

**Example:**
```bash
nutrition generate ingredient \
  --quantity 100g --quantity "1 cup" \
  --alias "brown rice" \
  --property "calories:370" \
  --property "protein:7.9g"
```

Output:
```nutrition
@ingredient(100g)(1 cup) "brown rice" {
    calories: 370
    protein: 7.9g
}
```

#### `generate recipe`

```bash
nutrition generate recipe \
  --quantity <QUANTITY> \
  --alias <ALIAS> \
  [--ingredient "<ALIAS>(<QUANTITY>)"]
```

| Flag | Description |
|------|-------------|
| `-q, --quantity <QUANTITY>` | Yield quantity — required |
| `-a, --alias <ALIAS>` | Recipe name(s) — required |
| `--ingredient <SPEC>` | Ingredient in `"alias"(quantity)` format — repeat for multiple |

**Example:**
```bash
nutrition generate recipe \
  --quantity 4 \
  --alias "lentil soup" \
  --ingredient '"lentils"(200g)' \
  --ingredient '"water"(500ml)'
```

#### `generate day`

```bash
nutrition generate day \
  --date <YYYY-MM-DD> \
  [--ate "<ALIAS>(<QUANTITY>)"] \
  [--exercised "<ALIAS>(<QUANTITY>)"]
```

| Flag | Description |
|------|-------------|
| `-d, --date <DATE>` | Date for the entry (`YYYY-MM-DD`) — required |
| `-a, --ate <SPEC>` | Food entry in `"alias"(quantity)` format — repeat for multiple |
| `-e, --exercised <SPEC>` | Exercise entry in `"alias"(quantity)` format — repeat for multiple |

**Example:**
```bash
nutrition generate day \
  --date 2026-01-15 \
  --ate '"oatmeal"(2)' \
  --exercised '"running"(30 min)'
```

Output:
```nutrition
@day "2026-01-15" {
    @ate "oatmeal"(2)
    @exercised "running"(30 min)
}
```

---

### `serve`

Start a local HTTP server that converts JSON-encoded `Item` objects into formatted Nutrition language text. Useful for programmatic code generation.

```bash
nutrition serve [--port <PORT>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-p, --port <PORT>` | `8080` | Port to listen on |

Send a `POST` request to `/` with a JSON body representing any `Item` variant:

```bash
curl -s -X POST http://localhost:8080 \
  -H "Content-Type: application/json" \
  -d '{
    "Ingredient": {
      "aliases": ["oats"],
      "quantities": [{"amount": 100, "unit": "g"}],
      "properties": [
        {"name": "calories", "value": {"amount": 389, "unit": null}}
      ]
    }
  }'
```

Response:
```nutrition
@ingredient(100g) "oats" {
    calories: 389
}
```

---

## VS Code Extension

The VS Code extension provides first-class editing support for `.nutrition` files.

### Features

| Feature | Description |
|---------|-------------|
| **Syntax highlighting** | Color-codes `@` keywords, strings, numbers, properties, and comments |
| **Semantic highlighting** | Token-level highlighting using tree-sitter for `@ingredient`, `@recipe`, `@exercise`, `@day`, `@ate`, `@exercised`, and more |
| **Find commands** | Quick-pick navigation to any ingredient, recipe, food, or day in the file (`Ctrl+Shift+P` → `Nutrition: Find …`) |
| **Import-aware navigation** | `Find` commands traverse `!import` chains and include declarations from imported `.nutrition` files |
| **Today command** | Jump to — or scaffold — today's `@day` entry (`Ctrl+Shift+P` → `Nutrition: Today`) |
| **Document formatting** | Auto-indentation and consistent block spacing (`Shift+Alt+F`) |

`Find Ingredient`, `Find Food`, and `Find Recipe` support both inline aliases and newline-delimited alias headers.

### Installation

1. Download the latest `.vsix` package from the [Releases](https://github.com/jafayer/nutrition-rs/releases) page (if available), or build it from source (see below).
2. Install it in VS Code:
   ```bash
   code --install-extension nutrition-language-0.1.0.vsix
   ```
3. Open any `.nutrition` file — the extension activates automatically.

### Building from Source

```bash
# From the repository root
./build-extension.sh
```

This installs npm dependencies and compiles the TypeScript source. To also produce an installable `.vsix` package:

```bash
cd vscode-extension
npm run package
# Creates nutrition-language-0.1.0.vsix
code --install-extension nutrition-language-0.1.0.vsix
```

To develop the extension interactively:

1. Open the extension folder in VS Code:
   ```bash
   code vscode-extension/
   ```
2. Press **F5** to launch an *Extension Development Host* window.
3. Open a `.nutrition` file in the new window to activate all features.

For continuous compilation during development:

```bash
cd vscode-extension
npm run watch
```

---

## GitHub Syntax Highlighting

### How Linguist works

GitHub uses [Linguist](https://github.com/github-linguist/linguist) to detect file languages and apply syntax highlighting. Linguist maintains a central database (`languages.yml`) of every language it knows about; when a file is highlighted on GitHub.com, Linguist looks up the language in that database and uses the grammar that was bundled into Linguist when the language was added.

**`.gitattributes` cannot reference a local grammar file.** The `*.nutrition linguist-language=Nutrition` entry in this repo's `.gitattributes` is a forward-looking setting — it tells Linguist "classify `.nutrition` files as the Nutrition language" — but it has no effect until `Nutrition` is added to Linguist's own database. There is no mechanism in `.gitattributes` to point GitHub to a local `.tmLanguage.json` file.

### What is needed for GitHub.com to render the grammar

To get `.nutrition` files and `` ```nutrition `` code fences highlighted on GitHub.com:

1. **Submit a PR to [github-linguist/linguist](https://github.com/github-linguist/linguist)** adding Nutrition as a new language. The Linguist [contributing guide](https://github.com/github-linguist/linguist/blob/master/CONTRIBUTING.md) describes the process in full. The key steps are:

   a. Add an entry to `languages.yml`. The grammar's TextMate scope name is `source.nutrition` (from `vscode-extension/syntaxes/nutrition.tmLanguage.json`):

   ```yaml
   Nutrition:
     type: data
     color: "#6B8E23"
     extensions:
     - ".nutrition"
     tm_scope: source.nutrition
     ace_mode: text
     language_id: <generated by script/update-ids>
   ```

   b. Register the grammar by running Linguist's helper script from inside a clone of the linguist repo, pointing it at this repository (where the grammar lives):

   ```bash
   script/add-grammar https://github.com/jafayer/nutrition-rs
   ```

   Linguist's `add-grammar` script will locate `vscode-extension/syntaxes/nutrition.tmLanguage.json` automatically because the file's `scopeName` field (`source.nutrition`) matches the `tm_scope` in the `languages.yml` entry.

   c. Add one or more sample `.nutrition` files to `samples/Nutrition/` in the linguist repo.

   d. Run `script/update-ids` to assign a unique `language_id`.

   e. Open a PR, linking to evidence of in-the-wild usage.

2. **Once merged**, GitHub will bundle the grammar in its next Linguist release, after which:
   - `.nutrition` files viewed on GitHub.com will be syntax-highlighted using the grammar at `vscode-extension/syntaxes/nutrition.tmLanguage.json`.
   - `` ```nutrition `` code fences in Markdown files will also be highlighted.
   - The `*.nutrition linguist-language=Nutrition` line in `.gitattributes` will become active, explicitly pinning any `.nutrition` file to the Nutrition language.

### Current state

| Context | Status |
|---------|--------|
| VS Code (extension installed) | ✅ Full syntax + semantic highlighting via `vscode-extension/syntaxes/nutrition.tmLanguage.json` |
| VS Code Markdown preview (extension installed) | ✅ `` ```nutrition `` fences highlighted |
| GitHub.com file view | ⏳ Requires Linguist contribution |
| GitHub.com Markdown code fences | ⏳ Requires Linguist contribution |

---

## Examples

A complete sample file demonstrating all language features:

```nutrition
// ── Ingredients ──────────────────────────────────────────────────────────────

@ingredient(100g)(1 cup) "chickpeas" "chickpea" "garbanzo beans" {
  calories: 269
  protein: 14.5g
  fat: 4g
  carbohydrates: 45g
  fiber: 12.5g
}

@ingredient(1 cup) "water" {}

@ingredient(1 pie)(8 slices) "pizza" {
  calories: 285
  protein: 12g
  fat: 10g
  carbohydrates: 36g
}

// ── Exercises ─────────────────────────────────────────────────────────────────

@exercise(30 min) "running" {
  calories: 300kcal
}

// ── Recipes ───────────────────────────────────────────────────────────────────

@recipe(8)(500g) "chickpea stew" {
  "chickpeas"(2 cups)
  "water"(5 cups)
}

@recipe(4) "simple chickpeas" {
  "water"(1 cup)
  "chickpeas"(200g)
}

// ── Days ──────────────────────────────────────────────────────────────────────

@day "2026-01-01" {
  @ate "chickpea stew"(2)
}

@day "2026-01-06" {
  @ate "simple chickpeas"(3)
  @exercised "running"(30 min)
}
```

See [`examples/test.nutrition`](examples/test.nutrition) for a runnable sample.

---

## License

See [LICENSE](LICENSE).
