# Manual test checklist

CI builds and runs the automated tests on macOS, Linux and Windows. These checks need a person at a real desktop, on each platform before a release. Run the app with `npm run tauri build` (or `npm run tauri dev`) from `app/`.

## Daemon lifecycle

- [ ] With no daemon running, opening the app starts one (`daemon.json` appears in the user data folder).
- [ ] Closing the app leaves the daemon and its sessions running; reopening shows them with the screen restored.
- [ ] Opening the app from the system launcher (not a terminal) still finds `claude` (the empty state shows no "Claude Code não encontrado" note).
- [ ] "Encerrar tudo" interrupts every session and the app comes back with them as Interrompida.
- [ ] Killing the daemon process shows the error screen; "Tentar de novo" and "Reiniciar daemon" recover.
- [ ] After rebuilding or updating the app while the old daemon keeps running, a banner offers "Reiniciar daemon"; "Agora não" hides it until the next outdated daemon, and restarting brings the sessions back as Interrompida.

## Sessions

- [ ] New Claude and shell sessions start in the chosen folder; the folder picker opens.
- [ ] Typing, pasting, resizing the window and full screen TUIs (Claude Code, `vim`, `htop`) render correctly.
- [ ] `exit` in a shell shows the Encerrada panel with the exit code; Retomar opens a new shell in the same folder.
- [ ] Restarting the computer: sessions come back as Interrompida and a Claude session resumes its conversation.
- [ ] A quiet session turns Ociosa after about 10 seconds and back to Rodando on output.
- [ ] Cmd+click (Ctrl+click) shows up to four terminals side by side; typing goes to the focused one.
- [ ] In a split, dragging a divider resizes columns and rows (double click or Enter evens them out); dragging a pane by its grip onto another swaps their places, and the grip's arrow keys do the same.
- [ ] Reopening the app brings back the split, its sizes and the focused session; sessions that ended meanwhile are left out.
- [ ] Cmd+1 to Cmd+9 (Ctrl elsewhere) open the active sessions in sidebar order, also from a focused terminal.
- [ ] Cmd+W (Ctrl+Shift+W elsewhere) closes the focused pane of a split and does nothing with a single pane; Shift+Cmd+W closes the window on macOS. Copy and paste still work in dialogs and terminals.
- [ ] Renaming from the pencil next to the title, or a double click in the sidebar, updates every open window.
- [ ] When Claude finishes an answer or asks for a permission, its session shows "Aguardando você" and, if it is not on screen or the window is in the background, a system notification appears (macOS asks for permission once). Typing in the session clears the mark.
- [ ] Cmd+J (Ctrl+Shift+J elsewhere) jumps to the next session showing "Aguardando você", in sidebar order and wrapping around, also from a focused terminal. With none waiting it does nothing.
- [ ] macOS: the Dock icon shows a badge with the number of sessions waiting, and the badge goes away when none waits.

## Projects

- [ ] A project with another Claude account opens Claude logged into that account.
- [ ] A project with an SSH key makes `git fetch` in its shells use that key.
- [ ] Rules and plan saved in "Regras e plano" reach a new Claude session of the project (ask Claude what the rules say), and an edit made between sessions reaches the session after Retomar.
- [ ] With a context folder of your own, the files are written there and "Mostrar pasta" opens it.

## Worktrees

- [ ] "Isolar em worktree" on a git repository starts the session in a new folder on a `cohors/...` branch, shown in the header; a folder outside git shows an error.
- [ ] After finishing, the Worktree card blocks removal while there are uncommitted changes, removes a clean worktree with its branch when it has no new commits, and keeps the branch when it has.
- [ ] Windows: paths with spaces and the removal work (git for Windows in PATH).

## Other agents

- [ ] Nova sessão disables Codex and Gemini when their CLI is not in PATH, and enables them after installing and restarting the daemon.
- [ ] A Codex session starts `codex`; after Encerrar and Retomar it continues the same conversation (`codex resume --last`).
- [ ] A Gemini session starts `gemini`; Retomar continues the same conversation (`gemini --resume latest`).
- [ ] Two Codex sessions in the same folder: resuming the older one opens the newer conversation (known limit). With worktrees each resumes its own.

## Finish and history

- [ ] Finalizar on a Claude session shows the progress screen and then the summary.
- [ ] A summary failure offers Tentar de novo and Finalizar sem resumo.
- [ ] Copiar resumo copies the text; Excluir removes the session.
- [ ] Ver transcript of a Claude session opens on Conversa (your prompts, Claude's answers with code blocks, one line per tool) and Terminal shows the raw output; a shell only has Terminal.
- [ ] Detalhes of a finished Claude session shows the model, token counts and Custo estimado in US$ (hover explains it is the API list price).
- [ ] The search box in Finalizadas finds sessions by name, notes, summary and conversation text; Esc clears it.

## Accessibility

- [ ] Every control is reachable with Tab and shows a focus ring.
- [ ] F6 leaves a focused terminal.
- [ ] Dialogs keep focus inside, close with Esc and return focus to what opened them.
- [ ] With "reduce motion" on, the summary spinner does not spin.

## Platform notes

- **Windows**: the project context hook runs through Claude Code's shell (Git Bash); check that the rules reach a Claude session. Sessions run under ConPTY; check that `claude` and PowerShell start, that resize works, and that "Encerrar tudo" ends child processes. Stopping the daemon from outside is a hard kill there, so sessions come back as Interrompida.
- **Linux**: check under both X11 and Wayland; the app needs WebKitGTK 4.1.
