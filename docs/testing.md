# Manual test checklist

CI builds and runs the automated tests on macOS, Linux and Windows. These checks need a person at a real desktop, on each platform before a release. Run the app with `npm run tauri build` (or `npm run tauri dev`) from `app/`.

## Daemon lifecycle

- [ ] With no daemon running, opening the app starts one (`daemon.json` appears in the user data folder).
- [ ] Closing the app leaves the daemon and its sessions running; reopening shows them with the screen restored.
- [ ] Opening the app from the system launcher (not a terminal) still finds `claude` (the empty state shows no "Claude Code não encontrado" note).
- [ ] "Encerrar tudo" interrupts every session and the app comes back with them as Interrompida.
- [ ] Killing the daemon process shows the error screen; "Tentar de novo" and "Reiniciar daemon" recover.

## Sessions

- [ ] New Claude and shell sessions start in the chosen folder; the folder picker opens.
- [ ] Typing, pasting, resizing the window and full screen TUIs (Claude Code, `vim`, `htop`) render correctly.
- [ ] `exit` in a shell shows the Encerrada panel with the exit code; Retomar opens a new shell in the same folder.
- [ ] Restarting the computer: sessions come back as Interrompida and a Claude session resumes its conversation.
- [ ] A quiet session turns Ociosa after about 10 seconds and back to Rodando on output.
- [ ] Cmd+click (Ctrl+click) shows up to four terminals side by side; typing goes to the focused one.

## Projects

- [ ] A project with another Claude account opens Claude logged into that account.
- [ ] A project with an SSH key makes `git fetch` in its shells use that key.
- [ ] Rules and plan saved in "Regras e plano" reach a new Claude session of the project (ask Claude what the rules say), and an edit made between sessions reaches the session after Retomar.
- [ ] With a context folder of your own, the files are written there and "Mostrar pasta" opens it.

## Finish and history

- [ ] Finalizar on a Claude session shows the progress screen and then the summary.
- [ ] A summary failure offers Tentar de novo and Finalizar sem resumo.
- [ ] Copiar resumo copies the text; Ver transcript shows the session output; Excluir removes it.

## Accessibility

- [ ] Every control is reachable with Tab and shows a focus ring.
- [ ] F6 leaves a focused terminal.
- [ ] Dialogs keep focus inside, close with Esc and return focus to what opened them.
- [ ] With "reduce motion" on, the summary spinner does not spin.

## Platform notes

- **Windows**: the project context hook runs through Claude Code's shell (Git Bash); check that the rules reach a Claude session. Sessions run under ConPTY; check that `claude` and PowerShell start, that resize works, and that "Encerrar tudo" ends child processes. Stopping the daemon from outside is a hard kill there, so sessions come back as Interrompida.
- **Linux**: check under both X11 and Wayland; the app needs WebKitGTK 4.1.
