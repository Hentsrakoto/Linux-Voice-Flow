function shortcutKeys(shortcut: string): string[] {
  return shortcut.split('+').map(key => ({ control: 'Ctrl', ctrl: 'Ctrl', commandorcontrol: 'Ctrl', alt: 'Alt', shift: 'Maj', space: 'Espace', super: 'Super', meta: 'Super' })[key.toLowerCase()] ?? key.replace(/^Key/, ''));
}
export default function ShortcutKeys({ shortcut }: { shortcut: string }) {
  return <div className="shortcut-keys" aria-label={shortcutKeys(shortcut).join(' + ')}>{shortcutKeys(shortcut).map((key, index) => <kbd key={`${index}-${key}`}>{key}</kbd>)}</div>;
}
