import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import HotkeyProbe from "./HotkeyProbe";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
const initial = {
  revision: 0,
  shortcut: "Ctrl+Alt+Space",
  held: false,
  supported: true,
  error: null,
  press_count: 0,
  release_count: 0,
};
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
  vi.mocked(listen).mockResolvedValue(vi.fn());
});
it("affiche les appuis et relâchements reçus depuis Rust", async () => {
  vi.mocked(invoke).mockResolvedValue(initial);
  render(<HotkeyProbe />);
  expect(await screen.findByText("Ctrl+Alt+Space")).toBeInTheDocument();
  expect(listen).toHaveBeenCalledWith("hotkey://event", expect.any(Function));
  expect(screen.getByText("Relâché")).toBeInTheDocument();
  act(() => {
    vi.mocked(listen).mock.calls[0][1]({
      event: "hotkey://event",
      id: 1,
      payload: { ...initial, revision: 2, held: true, press_count: 1 },
    });
  });
  expect(screen.getByText("Appuyé")).toBeInTheDocument();
  expect(screen.getByText("1 / 0")).toBeInTheDocument();
});
it("conserve le raccourci existant quand un remplacement échoue", async () => {
  vi.mocked(invoke)
    .mockResolvedValueOnce(initial)
    .mockRejectedValueOnce("Raccourci indisponible.");
  render(<HotkeyProbe />);
  await screen.findByText("Ctrl+Alt+Space");
  await userEvent.click(screen.getByText("Test du raccourci X11"));
  await userEvent.clear(screen.getByLabelText("Nouveau raccourci"));
  await userEvent.type(
    screen.getByLabelText("Nouveau raccourci"),
    "Ctrl+Alt+K",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Appliquer le raccourci" }),
  );
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Raccourci indisponible.",
  );
  expect(screen.getByText("Ctrl+Alt+Space")).toBeInTheDocument();
});
it("désabonne aussi une inscription résolue après démontage", async () => {
  let resolve!: (value: () => void) => void;
  const unlisten = vi.fn();
  vi.mocked(listen).mockImplementation(
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  vi.mocked(invoke).mockResolvedValue(initial);
  const view = render(<HotkeyProbe />);
  view.unmount();
  resolve(unlisten);
  await Promise.resolve();
  expect(unlisten).toHaveBeenCalledOnce();
});
it("ne remplace pas un événement récent par la réponse initiale tardive", async () => {
  let resolve!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((r) => {
        resolve = r;
      }) as ReturnType<typeof invoke>,
  );
  render(<HotkeyProbe />);
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    vi.mocked(listen).mock.calls[0][1]({
      event: "hotkey://event",
      id: 1,
      payload: { ...initial, revision: 2, held: true, press_count: 1 },
    });
    resolve(initial);
    await Promise.resolve();
  });
  expect(screen.getByText("Appuyé")).toBeInTheDocument();
  expect(screen.getByText("1 / 0")).toBeInTheDocument();
});

it("conserve un appui récent malgré une réponse de remplacement plus ancienne", async () => {
  let resolve!: (value: unknown) => void;
  vi.mocked(invoke)
    .mockResolvedValueOnce(initial)
    .mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }) as ReturnType<typeof invoke>,
    );
  render(<HotkeyProbe />);
  await screen.findByText("Ctrl+Alt+Space");
  await userEvent.click(screen.getByText("Test du raccourci X11"));
  await userEvent.clear(screen.getByLabelText("Nouveau raccourci"));
  await userEvent.type(
    screen.getByLabelText("Nouveau raccourci"),
    "Ctrl+Alt+K",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Appliquer le raccourci" }),
  );
  await act(async () => {
    vi.mocked(listen).mock.calls[0][1]({
      event: "hotkey://event",
      id: 1,
      payload: {
        ...initial,
        revision: 2,
        shortcut: "Ctrl+Alt+K",
        held: true,
        press_count: 1,
      },
    });
    resolve({ ...initial, revision: 1, shortcut: "Ctrl+Alt+K" });
    await Promise.resolve();
  });
  expect(screen.getByText("Appuyé")).toBeInTheDocument();
  expect(screen.getByText("1 / 0")).toBeInTheDocument();
  expect(screen.getByText("Ctrl+Alt+K")).toBeInTheDocument();
});
it("prévient que le raccourci enregistre lorsque le moteur est prêt", async () => {
  vi.mocked(invoke).mockResolvedValue(initial);
  render(<HotkeyProbe />);
  expect(await screen.findByText(/Lorsque le moteur est prêt, maintenir le raccourci enregistre/)).toBeInTheDocument();
});
it("affiche le repli Annuler si Échap ne peut pas être enregistré", async () => {
  vi.mocked(invoke).mockResolvedValue(initial);
  render(<HotkeyProbe />);
  await screen.findByText("Ctrl+Alt+Space");
  act(() => {
    vi.mocked(listen).mock.calls[1][1]({event: "dictation://notice", id: 2, payload: "Échap indisponible. Utilisez le bouton Annuler."});
  });
  expect(screen.getByText("Échap indisponible. Utilisez le bouton Annuler.")).toBeInTheDocument();
});
