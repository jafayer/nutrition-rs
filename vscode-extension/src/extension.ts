import * as path from 'path';
import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
  console.log('Nutrition language extension is now active');

  // Get the LSP server path from configuration or use default
  const config = vscode.workspace.getConfiguration('nutrition');
  let serverPath = config.get<string>('lsp.path');

  if (!serverPath) {
    // Use the bundled LSP server
    // Assuming the LSP binary is in the extension's bin directory
    const platform = process.platform;
    const extension = platform === 'win32' ? '.exe' : '';
    serverPath = context.asAbsolutePath(
      path.join('bin', `nutrition-lsp${extension}`)
    );
  }

  console.log(`Using LSP server at: ${serverPath}`);

  // Define the server executable
  const serverExecutable: Executable = {
    command: serverPath,
    args: [],
    options: {
      env: process.env
    }
  };

  const serverOptions: ServerOptions = {
    run: serverExecutable,
    debug: serverExecutable
  };

  // Options to control the language client
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'nutrition' }],
    synchronize: {
      // Notify the server about file changes to '.nutrition' files in the workspace
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.nutrition')
    },
    outputChannelName: 'Nutrition Language Server',
    traceOutputChannel: vscode.window.createOutputChannel('Nutrition LSP Trace'),
    middleware: {
      provideDocumentSemanticTokens: async (document, token, next) => {
        const result = await next(document, token);
        return result;
      }
    }
  };

  // Create the language client
  client = new LanguageClient(
    'nutritionLanguageServer',
    'Nutrition Language Server',
    serverOptions,
    clientOptions
  );

  // Start the client (which also launches the server)
  client.start();

  console.log('Nutrition LSP client started');
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
