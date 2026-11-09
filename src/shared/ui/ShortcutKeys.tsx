function shortcutKeys(shortcut: string): string[] {
  return shortcut.split('+').map(key => ({ Control: 'Ctrl', CommandOrControl: 'Ctrl', Space: 'Espace', Super: 'Super', Meta: 'Super' })[key] ?? key.replace(/^Key/, ''));
}
export default function ShortcutKeys({ shortcut }: { shortcut: string }) {
  return <div className="shortcut-keys" aria-label={shortcutKeys(shortcut).join(' + ')}>{shortcutKeys(shortcut).map((key, index) => <kbd key={`${index}-${key}`}>{key}</kbd>)}</div>;
}
