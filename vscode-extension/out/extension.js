"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const quoteRegexGlobal = /"([^"]*)"/g;
function extractQuotedStrings(block) {
    const aliases = [];
    let m;
    quoteRegexGlobal.lastIndex = 0; // reset just in case
    while ((m = quoteRegexGlobal.exec(block)) !== null) {
        aliases.push(m[1]);
    }
    return aliases;
}
function escapeRegex(value) {
    return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
function todayIsoDate() {
    return new Date().toISOString().slice(0, 10);
}
const DECL_CONFIGS = [
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
function parseDeclarations(document, config) {
    const results = [];
    for (let i = 0; i < document.lineCount; i++) {
        const line = document.lineAt(i);
        const match = line.text.match(config.regex);
        if (!match)
            continue;
        const aliases = config.extractAliases(match).filter(Boolean);
        if (aliases.length === 0)
            continue;
        results.push({
            label: aliases[0],
            line: i,
            aliases,
            kind: config.kind
        });
    }
    return results;
}
function findDayLine(document, isoDate) {
    const pattern = new RegExp(`@day\\s+"${escapeRegex(isoDate)}"`);
    for (let i = 0; i < document.lineCount; i++) {
        if (pattern.test(document.lineAt(i).text)) {
            return i;
        }
    }
    return undefined;
}
async function appendDayBlock(editor, isoDate) {
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
async function jumpToLine(editor, line) {
    const position = new vscode.Position(line, 0);
    editor.selection = new vscode.Selection(position, position);
    editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
}
function registerFindCommand(context, config) {
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
function registerTodayCommand(context) {
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
function activate(context) {
    console.log('Nutrition language extension is now active');
    DECL_CONFIGS.forEach(cfg => registerFindCommand(context, cfg));
    registerTodayCommand(context);
}
function deactivate() { }
//# sourceMappingURL=extension.js.map