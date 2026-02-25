import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

type DeclarationKind = 'ingredient' | 'food' | 'recipe' | 'day';

interface DeclMatch {
  label: string;
  line: number;
  aliases: string[];
  kind: DeclarationKind;
  uri: vscode.Uri;
  displayPath: string;
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
    emptyMessage: 'No ingredients found in this file or imported files'
  },
  {
    kind: 'food',
    commandId: 'nutrition.findFood',
    title: 'Find Food',
    regex: /@food\s*(?:\([^)]*\))*\s*((?:"[^"]*"\s*)+)/,
    extractAliases: match => extractQuotedStrings(match[1] ?? ''),
    placeholder: 'Find food...',
    emptyMessage: 'No foods found in this file or imported files'
  },
  {
    kind: 'recipe',
    commandId: 'nutrition.findRecipe',
    title: 'Find Recipe',
    regex: /@recipe\s*(?:\([^)]*\))*\s*((?:"[^"]*"\s*)+)/,
    extractAliases: match => extractQuotedStrings(match[1] ?? ''),
    placeholder: 'Find recipe...',
    emptyMessage: 'No recipes found in this file or imported files'
  },
  {
    kind: 'day',
    commandId: 'nutrition.findDay',
    title: 'Find Day',
    regex: /@day\s+"([^"]+)",?/,
    extractAliases: match => (match[1] ? [match[1]] : []),
    placeholder: 'Find day...',
    emptyMessage: 'No days found in this file or imported files'
  }
];

function parseDeclarations(document: vscode.TextDocument, config: DeclConfig): DeclMatch[] {
  return parseDeclarationsFromText(document.getText(), document.uri, config);
}

function parseDeclarationsFromText(text: string, uri: vscode.Uri, config: DeclConfig): DeclMatch[] {
  const results: DeclMatch[] = [];
  const displayPath = getDisplayPath(uri);
  const lines = text.split(/\r?\n/);
  const directive = `@${config.kind}`;

  if (config.kind === 'day') {
    for (let i = 0; i < lines.length; i++) {
      const aliases = extractDeclarationAliasesFromLine(lines[i], config.kind);
      if (aliases.length === 0) continue;

      results.push({
        label: aliases[0],
        line: i,
        aliases,
        kind: config.kind,
        uri,
        displayPath
      });
    }

    return results;
  }

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trimStart();
    if (!trimmed.startsWith(directive)) {
      continue;
    }

    const startLine = i;
    let headerText = lines[i];
    let endLine = i;

    while (!headerText.includes('{') && endLine + 1 < lines.length) {
      const nextLine = lines[endLine + 1];
      const nextTrimmed = nextLine.trimStart();

      if (nextTrimmed.startsWith('@')) {
        break;
      }

      headerText += `\n${nextLine}`;
      endLine += 1;
    }

    const headerSegment = headerText.split('{', 1)[0] ?? headerText;
    const aliases = extractQuotedStrings(headerSegment).filter(Boolean);
    if (aliases.length === 0) {
      continue;
    }

    results.push({
      label: aliases[0],
      line: startLine,
      aliases,
      kind: config.kind,
      uri,
      displayPath
    });

    i = endLine;
  }

  return results;
}

function extractDeclarationAliasesFromLine(lineText: string, kind: DeclarationKind): string[] {
  const trimmed = lineText.trimStart();
  const directive = `@${kind}`;
  if (!trimmed.startsWith(directive)) {
    return [];
  }

  if (kind === 'day') {
    const match = trimmed.match(/^@day\s+"([^"]+)"/);
    return match?.[1] ? [match[1]] : [];
  }

  return extractQuotedStrings(trimmed).filter(Boolean);
}

function getDisplayPath(uri: vscode.Uri): string {
  if (uri.scheme !== 'file') {
    return uri.toString();
  }

  const relativePath = vscode.workspace.asRelativePath(uri, false);
  if (!relativePath || relativePath === uri.fsPath) {
    return path.basename(uri.fsPath);
  }

  return relativePath;
}

function getUriVisitKey(uri: vscode.Uri): string {
  if (uri.scheme !== 'file') {
    return uri.toString();
  }

  try {
    return fs.realpathSync.native(uri.fsPath);
  } catch {
    return path.resolve(uri.fsPath);
  }
}

function collectImportTargets(document: vscode.TextDocument): vscode.Uri[] {
  if (document.uri.scheme !== 'file') {
    return [];
  }

  const targets: vscode.Uri[] = [];
  const sourceDir = path.dirname(document.uri.fsPath);

  for (let line = 0; line < document.lineCount; line++) {
    const text = document.lineAt(line).text;
    const parsed = parseImportDirectiveLine(text);
    if (!parsed) {
      continue;
    }

    const resolvedPath = path.isAbsolute(parsed.importPath)
      ? parsed.importPath
      : path.resolve(sourceDir, parsed.importPath);

    if (!fs.existsSync(resolvedPath)) {
      continue;
    }

    targets.push(vscode.Uri.file(resolvedPath));
  }

  return targets;
}

function collectImportTargetsFromText(sourceUri: vscode.Uri, text: string): vscode.Uri[] {
  if (sourceUri.scheme !== 'file') {
    return [];
  }

  const targets: vscode.Uri[] = [];
  const sourceDir = path.dirname(sourceUri.fsPath);
  const lines = text.split(/\r?\n/);

  for (const lineText of lines) {
    const parsed = parseImportDirectiveLine(lineText);
    if (!parsed) {
      continue;
    }

    const resolvedPath = path.isAbsolute(parsed.importPath)
      ? parsed.importPath
      : path.resolve(sourceDir, parsed.importPath);

    if (!fs.existsSync(resolvedPath)) {
      continue;
    }

    targets.push(vscode.Uri.file(resolvedPath));
  }

  return targets;
}

async function collectSearchDocuments(root: vscode.TextDocument): Promise<vscode.TextDocument[]> {
  const discoveredDocuments: vscode.TextDocument[] = [];
  const queuedDocuments: vscode.TextDocument[] = [root];
  const visited = new Set<string>();

  while (queuedDocuments.length > 0) {
    const document = queuedDocuments.shift();
    if (!document) {
      continue;
    }

    const visitKey = getUriVisitKey(document.uri);
    if (visited.has(visitKey)) {
      continue;
    }

    visited.add(visitKey);
    discoveredDocuments.push(document);

    const imports = collectImportTargets(document);
    for (const importUri of imports) {
      const importKey = getUriVisitKey(importUri);
      if (visited.has(importKey)) {
        continue;
      }

      try {
        const importedDocument = await vscode.workspace.openTextDocument(importUri);
        if (importedDocument.languageId === 'nutrition' || importUri.fsPath.endsWith('.nutrition')) {
          queuedDocuments.push(importedDocument);
        }
      } catch {
      }
    }
  }

  return discoveredDocuments;
}

async function collectDeclarationsAcrossImports(document: vscode.TextDocument, config: DeclConfig): Promise<DeclMatch[]> {
  const searchDocuments = await collectSearchDocuments(document);
  const declarations: DeclMatch[] = [];

  for (const searchDocument of searchDocuments) {
    declarations.push(...parseDeclarations(searchDocument, config));
  }

  return declarations;
}

async function collectDeclarationsAcrossImportsFromUri(rootUri: vscode.Uri, config: DeclConfig): Promise<DeclMatch[]> {
  const declarations: DeclMatch[] = [];
  const queuedUris: vscode.Uri[] = [rootUri];
  const visited = new Set<string>();

  while (queuedUris.length > 0) {
    const uri = queuedUris.shift();
    if (!uri) {
      continue;
    }

    const visitKey = getUriVisitKey(uri);
    if (visited.has(visitKey)) {
      continue;
    }

    visited.add(visitKey);

    if (uri.scheme !== 'file') {
      continue;
    }

    let content = '';
    try {
      content = await fs.promises.readFile(uri.fsPath, 'utf8');
    } catch {
      continue;
    }

    declarations.push(...parseDeclarationsFromText(content, uri, config));

    for (const importUri of collectImportTargetsFromText(uri, content)) {
      if (importUri.fsPath.endsWith('.nutrition')) {
        queuedUris.push(importUri);
      }
    }
  }

  return declarations;
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

async function jumpToDocumentLine(uri: vscode.Uri, line: number) {
  const document = await vscode.workspace.openTextDocument(uri);
  const editor = await vscode.window.showTextDocument(document);
  await jumpToLine(editor, line);
}

function isNutritionDocument(document: vscode.TextDocument): boolean {
  return document.languageId === 'nutrition' || document.uri.fsPath.endsWith('.nutrition');
}

async function getActiveNutritionDocument(): Promise<vscode.TextDocument | undefined> {
  const activeEditor = vscode.window.activeTextEditor;
  if (activeEditor && isNutritionDocument(activeEditor.document)) {
    return activeEditor.document;
  }

  const activeTab = vscode.window.tabGroups.activeTabGroup.activeTab;
  if (activeTab?.input instanceof vscode.TabInputText) {
    try {
      const tabDocument = await vscode.workspace.openTextDocument(activeTab.input.uri);
      if (isNutritionDocument(tabDocument)) {
        return tabDocument;
      }
    } catch {
    }
  }

  const visibleNutritionEditor = vscode.window.visibleTextEditors.find(editor => isNutritionDocument(editor.document));
  if (visibleNutritionEditor) {
    return visibleNutritionEditor.document;
  }

  return vscode.workspace.textDocuments.find(isNutritionDocument);
}

async function getActiveNutritionUri(): Promise<vscode.Uri | undefined> {
  const activeEditor = vscode.window.activeTextEditor;
  if (activeEditor && isNutritionDocument(activeEditor.document)) {
    return activeEditor.document.uri;
  }

  const activeTab = vscode.window.tabGroups.activeTabGroup.activeTab;
  if (activeTab?.input instanceof vscode.TabInputText) {
    const uri = activeTab.input.uri;
    if (uri.scheme === 'file' && uri.fsPath.endsWith('.nutrition')) {
      return uri;
    }

    try {
      const tabDocument = await vscode.workspace.openTextDocument(uri);
      if (isNutritionDocument(tabDocument)) {
        return tabDocument.uri;
      }
    } catch {
    }
  }

  const visibleNutritionEditor = vscode.window.visibleTextEditors.find(editor => isNutritionDocument(editor.document));
  if (visibleNutritionEditor) {
    return visibleNutritionEditor.document.uri;
  }

  return vscode.workspace.textDocuments.find(isNutritionDocument)?.uri;
}

async function getActiveNutritionEditor(): Promise<vscode.TextEditor | undefined> {
  const activeEditor = vscode.window.activeTextEditor;
  if (activeEditor && isNutritionDocument(activeEditor.document)) {
    return activeEditor;
  }

  const activeDocument = await getActiveNutritionDocument();
  if (!activeDocument) {
    return undefined;
  }

  return vscode.window.showTextDocument(activeDocument, { preview: false });
}

function registerFindCommand(context: vscode.ExtensionContext, config: DeclConfig) {
  const disposable = vscode.commands.registerCommand(config.commandId, async () => {
    const uri = await getActiveNutritionUri();
    if (!uri) {
      vscode.window.showErrorMessage('No active nutrition editor');
      return;
    }

    const decls = await collectDeclarationsAcrossImportsFromUri(uri, config);
    if (decls.length === 0) {
      vscode.window.showInformationMessage(config.emptyMessage);
      return;
    }

    const items = decls.map(d => ({
      label: d.label,
      description: d.aliases.length > 1 ? `Also: ${d.aliases.slice(1).join(', ')}` : '',
      detail: `${d.displayPath}:${d.line + 1}`,
      decl: d
    }));

    const selected = await vscode.window.showQuickPick(items, {
      placeHolder: config.placeholder,
      matchOnDescription: true,
      matchOnDetail: true
    });

    if (selected) {
      await jumpToDocumentLine(selected.decl.uri, selected.decl.line);
    }
  });

  context.subscriptions.push(disposable);
}

function registerTodayCommand(context: vscode.ExtensionContext) {
  const disposable = vscode.commands.registerCommand('nutrition.today', async () => {
    const editor = await getActiveNutritionEditor();
    if (!editor) {
      vscode.window.showErrorMessage('No active nutrition editor');
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

function decodeStringEscape(ch: string): string {
  switch (ch) {
    case 'n':
      return '\n';
    case 'r':
      return '\r';
    case 't':
      return '\t';
    case '"':
      return '"';
    case '\\':
      return '\\';
    default:
      return ch;
  }
}

interface ImportPathMatch {
  importPath: string;
  startCol: number;
  endCol: number;
}

function parseImportDirectiveLine(lineText: string): ImportPathMatch | undefined {
  const match = lineText.match(/^(\s*!import\s+)("(?:[^"\\]|\\.)*")\s*(?:(\/\/).*)?$/);
  if (!match) {
    return undefined;
  }

  const prefix = match[1] ?? '';
  const quoted = match[2] ?? '';
  const startCol = prefix.length;
  const endCol = startCol + quoted.length;

  let idx = 1;
  let importPath = '';

  while (idx < quoted.length - 1) {
    const ch = quoted[idx];

    if (ch === '\\' && idx + 1 < quoted.length) {
      importPath += decodeStringEscape(quoted[idx + 1]);
      idx += 2;
      continue;
    }

    importPath += ch;
    idx += 1;
  }

  return { importPath, startCol, endCol };
}

function registerImportDocumentLinks(context: vscode.ExtensionContext) {
  const provider = vscode.languages.registerDocumentLinkProvider([
    { language: 'nutrition', scheme: 'file' },
    { language: 'nutrition', scheme: 'untitled' }
  ], {
    provideDocumentLinks(document: vscode.TextDocument): vscode.DocumentLink[] {
      const links: vscode.DocumentLink[] = [];

      for (let line = 0; line < document.lineCount; line++) {
        const text = document.lineAt(line).text;
        const parsed = parseImportDirectiveLine(text);
        if (!parsed) {
          continue;
        }

        const resolvedPath = path.isAbsolute(parsed.importPath)
          ? parsed.importPath
          : path.resolve(path.dirname(document.uri.fsPath), parsed.importPath);

        const targetUri = vscode.Uri.file(resolvedPath);
        const range = new vscode.Range(
          new vscode.Position(line, parsed.startCol),
          new vscode.Position(line, parsed.endCol)
        );

        const link = new vscode.DocumentLink(range, targetUri);
        if (!fs.existsSync(resolvedPath)) {
          link.tooltip = `Missing imported file: ${parsed.importPath}`;
        }
        links.push(link);
      }

      return links;
    }
  });

  context.subscriptions.push(provider);
}

function registerImportDefinitionProvider(context: vscode.ExtensionContext) {
  const provider = vscode.languages.registerDefinitionProvider('nutrition', {
    provideDefinition(document: vscode.TextDocument, position: vscode.Position): vscode.LocationLink[] {
      const text = document.lineAt(position.line).text;
      const parsed = parseImportDirectiveLine(text);
      if (!parsed) {
        return [];
      }

      if (position.character < parsed.startCol || position.character > parsed.endCol) {
        return [];
      }

      const resolvedPath = path.isAbsolute(parsed.importPath)
        ? parsed.importPath
        : path.resolve(path.dirname(document.uri.fsPath), parsed.importPath);

      if (!fs.existsSync(resolvedPath)) {
        return [];
      }

      const targetUri = vscode.Uri.file(resolvedPath);
      return [
        {
          targetUri,
          targetRange: new vscode.Range(new vscode.Position(0, 0), new vscode.Position(0, 0)),
          targetSelectionRange: new vscode.Range(new vscode.Position(0, 0), new vscode.Position(0, 0)),
          originSelectionRange: new vscode.Range(
            new vscode.Position(position.line, parsed.startCol),
            new vscode.Position(position.line, parsed.endCol)
          )
        }
      ];
    }
  });

  context.subscriptions.push(provider);
}

function getServerCommand(context: vscode.ExtensionContext): string {
  const config = vscode.workspace.getConfiguration('nutrition');
  const configuredPath = config.get<string>('lsp.path')?.trim();
  const serverPath = configuredPath && configuredPath.length > 0
    ? configuredPath
    : context.asAbsolutePath(path.join('bin', 'nutrition-lsp'));

  if (!fs.existsSync(serverPath)) {
    throw new Error(`Nutrition LSP binary not found at: ${serverPath}`);
  }

  return serverPath;
}

async function startLanguageClient(context: vscode.ExtensionContext): Promise<void> {
  const outputChannel = vscode.window.createOutputChannel('Nutrition Language Server');
  const traceOutputChannel = vscode.window.createOutputChannel('Nutrition LSP Trace');
  context.subscriptions.push(outputChannel, traceOutputChannel);

  const command = getServerCommand(context);
  const serverOptions: ServerOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio }
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'nutrition' }],
    outputChannel,
    traceOutputChannel
  };

  client = new LanguageClient('nutritionLanguageServer', 'Nutrition Language Server', serverOptions, clientOptions);

  const traceSetting = vscode.workspace
    .getConfiguration('nutrition')
    .get<string>('trace.server', 'off');

  if (traceSetting === 'messages') {
    client.setTrace(1);
  } else if (traceSetting === 'verbose') {
    client.setTrace(2);
  } else {
    client.setTrace(0);
  }

  await client.start();
}

function registerFormattingProvider(context: vscode.ExtensionContext) {
  const provider = vscode.languages.registerDocumentFormattingEditProvider('nutrition', {
    provideDocumentFormattingEdits(document: vscode.TextDocument): vscode.TextEdit[] {
      const edits: vscode.TextEdit[] = [];
      const indentSize = 2;
      
      let braceDepth = 0;
      let lastClosingBraceLine: number | undefined;
      
      for (let i = 0; i < document.lineCount; i++) {
        const line = document.lineAt(i);
        const text = line.text;
        const trimmed = text.trim();
        
        // Skip empty lines
        if (trimmed === '') {
          continue;
        }
        
        // Determine indentation for this line
        let expectedIndent = braceDepth * indentSize;
        
        // Closing braces decrease indent before the line
        if (trimmed.startsWith('}')) {
          expectedIndent = Math.max(0, (braceDepth - 1) * indentSize);
          lastClosingBraceLine = i;
        }
        
        // Check if this line starts a new block (after a closing brace)
        // and ensure there's a blank line between them
        const isBlockStart = /^@(ingredient|food|recipe|day|exercise)\b/.test(trimmed);
        if (isBlockStart && lastClosingBraceLine !== undefined && i === lastClosingBraceLine + 1) {
          // Insert a blank line before this block
          const insertPos = new vscode.Position(i, 0);
          edits.push(new vscode.TextEdit(new vscode.Range(insertPos, insertPos), '\n'));
        }
        
        const currentIndent = line.text.match(/^(\s*)/)?.[1]?.length ?? 0;
        const expectedIndentStr = ' '.repeat(expectedIndent);
        
        // Only add edit if indentation differs
        if (currentIndent !== expectedIndent) {
          const range = new vscode.Range(i, 0, i, currentIndent);
          edits.push(new vscode.TextEdit(range, expectedIndentStr));
        }
        
        // Update brace depth for next iteration
        // Count opening braces before any closing braces
        const openBraces = (text.match(/\{/g) ?? []).length;
        const closeBraces = (text.match(/\}/g) ?? []).length;
        braceDepth += openBraces - closeBraces;
      }
      
      return edits;
    }
  });
  
  context.subscriptions.push(provider);
}

export function activate(context: vscode.ExtensionContext) {
  console.log('Nutrition language extension is now active');

  DECL_CONFIGS.forEach(cfg => registerFindCommand(context, cfg));
  registerTodayCommand(context);
  registerImportDocumentLinks(context);
  registerImportDefinitionProvider(context);
  registerFormattingProvider(context);

  startLanguageClient(context).catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    vscode.window.showErrorMessage(`Failed to start Nutrition Language Server: ${message}`);
  });
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
