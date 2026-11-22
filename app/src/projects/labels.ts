import type { ClaudeAccount, Project, SshKey } from '@cohors/protocol';

export function accountLabel(account: ClaudeAccount): string {
  const owner = account.configDir === null ? 'Conta padrão' : (account.organization ?? folderName(account.configDir));
  return account.email === null ? owner : `${owner} · ${account.email}`;
}

/** Short description of a project's account and key for hints. */
export function environmentSummary(
  project: Project,
  accounts: ClaudeAccount[],
  keys: SshKey[],
): string {
  const account = accounts.find((candidate) => candidate.configDir === project.claudeConfigDir);
  const accountText =
    project.claudeConfigDir === null
      ? 'conta padrão do Claude'
      : (account?.organization ?? folderName(project.claudeConfigDir));
  const key = keys.find((candidate) => candidate.path === project.sshKey);
  const keyText =
    project.sshKey === null ? 'SSH padrão do sistema' : `chave ${key?.name ?? folderName(project.sshKey)}`;
  return `Usa ${accountText} e ${keyText}.`;
}

export function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}
