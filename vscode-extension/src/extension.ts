import * as vscode from 'vscode';

type DeclarationKind = 'ingredient' | 'food' | 'recipe' | 'day';

interface DeclMatch {
  label: string;
  line: number;
  aliases: string[];
  kind: DeclarationKind;
}

interface DeclConfig {
  kind: DeclarationKind;
  commandId: string;
  title: string;
  regex: RegExp;
  extractAliases: (match: RegExpMatchArray) => string[];
  placeholder: string;
  emptyMessage: string;
}

const quoteRegexGlobal = /"([^"]*)"/g;

function extractQuotedStrings(block: string): string[] {
  const aliases: string[] = [];
  let m: RegExpExecArray | null;
  quoteRegexGlobal.lastIndex = 0; // reset just in case
  while ((m = quoteRegexGlobal.exec(block)) !== null) {
    aliases.push(m[1]);
  }
  return aliases;
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function todayIsoDate(): string {
  return new Date().toISOString().slice(0, 10);
}

const DECL_CONFIGS: DeclConfig[] = [
  {
    kind: 'ingredient',
    commandId: 'nutrition.findIngredient',
    title: 'Find Ingredient',
    regex: /@ingredient\s*(?:\([^)]*\))*\s*((?:"[^"]*"\s*)+)/,
    extractAliases: match => extractQuotedStrings(match[1] ?? ''),
    placeholder: 'Find ingredient...',
    emptyMessage: 'No ingredients found in this file'
  },
  {
    kind: 'food',
    commandId: 'nutrition.findFood',
    title: 'Find Food',
    regex: /@food\s*(?:\([^)]*\))*\s*((?:"[^"]*"\s*)+)/,
    extractAliases: match => extractQuotedStrings(match[1] ?? ''),
    placeholder: 'Find food...',
    emptyMessage: 'No foods found in this file'
  },
  {
    kind: 'recipe',
    commandId: 'nutrition.findRecipe',
    title: 'Find Recipe',
    regex: /@recipe\s*(?:\([^)]*\))*\s*((?:"[^"]*"\s*)+)/,
    extractAliases: match => extractQuotedStrings(match[1] ?? ''),
    placeholder: 'Find recipe...',
    emptyMessage: 'No recipes found in this file'
  },
  {
    kind: 'day',
    commandId: 'nutrition.findDay',
    title: 'Find Day',
    regex: /@day\s+"([^"]+)",?/,
    extractAliases: match => (match[1] ? [match[1]] : []),
    placeholder: 'Find day...',
    emptyMessage: 'No days found in this file'
  }
];

function parseDeclarations(document: vscode.TextDocument, config: DeclConfig): DeclMatch[] {
  const results: DeclMatch[] = [];

  for (let i = 0; i < document.lineCount; i++) {
    const line = document.lineAt(i);
    const match = line.text.match(config.regex);
    if (!match) continue;

    const aliases = config.extractAliases(match).filter(Boolean);
    if (aliases.length === 0) continue;

    results.push({
      label: aliases[0],
      line: i,
      aliases,
      kind: config.kind
    });
  }

  return results;
}

function findDayLine(document: vscode.TextDocument, isoDate: string): number | undefined {
  const pattern = new RegExp(`@day\\s+"${escapeRegex(isoDate)}"`);
  for (let i = 0; i < document.lineCount; i++) {
    if (pattern.test(document.lineAt(i).text)) {
      return i;
    }
  }
  return undefined;
}

async function appendDayBlock(editor: vscode.TextEditor, isoDate: string): Promise<number> {
  const document = editor.document;
  const startLine = document.lineCount > 0 ? document.lineCount : 0;
  const needsLeadingNewline = document.lineCount > 0;
  const snippet = `${needsLeadingNewline ? '\n' : ''}@day "${isoDate}" {\n    \n}\n`;

  const insertPos = document.lineCount > 0
    ? document.lineAt(document.lineCount - 1).range.end
    : new vscode.Position(0, 0);

  const ok = await editor.edit(editBuilder => {
    editBuilder.insert(insertPos, snippet);
  });

  return ok ? startLine + (needsLeadingNewline ? 1 : 0) : startLine;
}

async function jumpToLine(editor: vscode.TextEditor, line: number) {
  const position = new vscode.Position(line, 0);
  editor.selection = new vscode.Selection(position, position);
  editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
}

function registerFindCommand(context: vscode.ExtensionContext, config: DeclConfig) {
  const disposable = vscode.commands.registerCommand(config.commandId, async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showErrorMessage('No active editor');
      return;
    }

    const decls = parseDeclarations(editor.document, config);
    if (decls.length === 0) {
      vscode.window.showInformationMessage(config.emptyMessage);
      return;
    }

    const items = decls.map(d => ({
      label: d.label,
      description: d.aliases.length > 1 ? `Also: ${d.aliases.slice(1).join(', ')}` : '',
      detail: `Line ${d.line + 1}`,
      decl: d
    }));

    const selected = await vscode.window.showQuickPick(items, {
      placeHolder: config.placeholder,
      matchOnDescription: true,
      matchOnDetail: true
    });

    if (selected) {
      await jumpToLine(editor, selected.decl.line);
    }
  });

  context.subscriptions.push(disposable);
}

function registerTodayCommand(context: vscode.ExtensionContext) {
  const disposable = vscode.commands.registerCommand('nutrition.today', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showErrorMessage('No active editor');
      return;
    }

    const isoDate = todayIsoDate();
    const existingLine = findDayLine(editor.document, isoDate);

    if (existingLine !== undefined) {
      await jumpToLine(editor, existingLine);
      return;
    }

    const line = await appendDayBlock(editor, isoDate);
    await jumpToLine(editor, line);
  });

  context.subscriptions.push(disposable);
}

export function activate(context: vscode.ExtensionContext) {
  console.log('Nutrition language extension is now active');

  DECL_CONFIGS.forEach(cfg => registerFindCommand(context, cfg));
  registerTodayCommand(context);
}

export function deactivate() {}
