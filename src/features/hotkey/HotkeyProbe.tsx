import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
interface HotkeyStatus {
  revision: number;
  shortcut: string | null;
  held: boolean;
  supported: boolean;
  error: string | null;
  press_count: number;
  release_count: number;
}
function latestSnapshot(
  current: HotkeyStatus | null,
  candidate: HotkeyStatus,
): HotkeyStatus {
  return !current || candidate.revision >= current.revision
    ? candidate
    : current;
}
export default function HotkeyProbe() {
  const [status, setStatus] = useState<HotkeyStatus | null>(null);
  const [shortcut, setShortcut] = useState("Ctrl+Alt+Space");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    const subscriptions: (() => void)[] = [];
    async function connect() {
      try {
        const stop = await listen<HotkeyStatus>("hotkey://event", (event) => {
          if (active)
            setStatus((current) => latestSnapshot(current, event.payload));
        });
        if (!active) {
          stop();
          return;
        }
        subscriptions.push(stop);
        const stopNotice = await listen<string>("dictation://notice", (event) => {
          if (active) setError(event.payload);
        });
        if (!active) {
          stopNotice();
          return;
        }
        subscriptions.push(stopNotice);
        const value = await invoke<HotkeyStatus>("get_hotkey_status");
        if (active) setStatus((current) => latestSnapshot(current, value));
      } catch {
        if (active)
          setError(
            "Impossible de vérifier le raccourci. Relancez l’application.",
          );
      }
    }
    void connect();
    return () => {
      active = false;
      subscriptions.forEach((stop) => stop());
    };
  }, []);
  async function apply(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!shortcut.trim()) {
      setError("Saisissez un raccourci.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const value = await invoke<HotkeyStatus>("replace_hotkey", {
        shortcut: shortcut.trim(),
      });
      setStatus((current) => latestSnapshot(current, value));
    } catch (reason: unknown) {
      setError(
        typeof reason === "string"
          ? reason
          : "Impossible d’appliquer le raccourci.",
      );
    } finally {
      setBusy(false);
    }
  }
  return (
    <details className="hotkey-probe">
      <summary>Test du raccourci X11</summary>
      <div className="probe-content">
        <p>
          Lorsque le moteur est prêt, maintenir le raccourci enregistre votre
          microphone ; le relâcher lance la transcription locale. Les compteurs
          ci-dessous servent au diagnostic. Échap annule pendant une opération ;
          si le raccourci d’annulation est indisponible, utilisez le bouton Annuler.
          Le raccourci est réinitialisé au redémarrage.
        </p>
        {status ? (
          <>
            <dl className="probe-metadata" aria-live="polite">
              <div>
                <dt>Raccourci actif</dt>
                <dd>{status.shortcut ?? "Aucun"}</dd>
              </div>
              <div>
                <dt>État</dt>
                <dd>{status.held ? "Appuyé" : "Relâché"}</dd>
              </div>
              <div>
                <dt>Appuis / relâchements</dt>
                <dd>
                  {status.press_count} / {status.release_count}
                </dd>
              </div>
            </dl>
            {!status.supported && (
              <p>Le test nécessite une session X11 compatible.</p>
            )}
            {status.error && <p role="alert">{status.error}</p>}
            {status.supported && (
              <form
                aria-busy={busy}
                noValidate
                onSubmit={(event) => {
                  void apply(event);
                }}
              >
                <label htmlFor="shortcut">Nouveau raccourci</label>
                <div className="shortcut-controls">
                  <input
                    id="shortcut"
                    value={shortcut}
                    onChange={(event) => setShortcut(event.target.value)}
                    aria-invalid={error === "Saisissez un raccourci."}
                    aria-describedby={error ? "shortcut-error" : undefined}
                    maxLength={128}
                    required
                    disabled={busy}
                  />
                  <button disabled={busy} type="submit">
                    {busy ? "Application…" : "Appliquer le raccourci"}
                  </button>
                </div>
              </form>
            )}
          </>
        ) : (
          <p>
            {error
              ? "Statut du raccourci indisponible."
              : "Vérification du raccourci…"}
          </p>
        )}
        {error && (
          <p id="shortcut-error" role="alert">
            {error}
          </p>
        )}
      </div>
    </details>
  );
}
