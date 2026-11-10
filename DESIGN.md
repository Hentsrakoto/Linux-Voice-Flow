---
version: beta
colors:
  paper: "#f5f6f2"
  surface: "#ffffff"
  ink: "#203932"
  muted: "#52665e"
  primary: "#176a57"
  line: "#d5dfd8"
typography:
  sans:
    fontFamily: '"Segoe UI", "Noto Sans", system-ui, sans-serif'
  mono:
    fontFamily: '"DejaVu Sans Mono", monospace'
rounded:
  panel: "16px"
  control: "6px"
omitted:
  - section: spacing
    reason: "Runtime spacing is owned by the shared stylesheet and Tailwind layout utilities."
  - section: components
    reason: "Native controls and shared CSS classes own the small desktop utility's primitives."
---

## Overview

A French desktop dictation utility: the main task is dictating into the current external application. A quiet Linux instrument panel, with the rounded green microphone-and-wave mark beside the Voice Flow name and a row of physical-looking shortcut keys as the signature. The main view has no configuration form or diagnostic grid. No promotional hero, simulated capture or fake transcription.

## Colors and runtime mapping

The six named frontmatter colors map exactly to `--color-*` inside the `@theme` block in `src/styles.css`; that shared stylesheet is the runtime token owner. Ink and muted carry text; primary carries focus and active state. One light theme, flat surfaces, one border, no shadows or gradients. Never rely on color alone for state.

## Typography

System sans for product copy and headings, system mono for shortcut keys and metadata. No remote fonts. Heading 28–38px, normal copy 13–16px, metadata at least 12px. French accents remain intact. Shortcut keys are the only enlarged utility typography.

## Layout and navigation

`App` owns a two-view navigation: Dictée and Réglages. Both are regular views, not dialogs. The shared runtime hook stays mounted across navigation, so asynchronous state and transcription retention remain backend-owned. App also owns the unsaved settings draft; switching views does not discard edits. Focus moves to main content only on an explicit view change, never on a dictation status update. Main maximum760px inside a1040px shell; reduce padding at600px. Allow page scrolling, wrap long status/error text, keep result height stable. Navigation uses semantic buttons with `aria-current`, visible keyboard focus and a skip link.

Dictée shows the actual registered shortcut, runtime status, service privacy indication, cancellation and the ephemeral result. It does not contain model, microphone, language, shortcut or provider configuration. Réglages owns all configuration and a diagnostic disclosure with corrective actions for missing X11, selected microphone, model configuration, xdotool or xclip. Dictée keeps a short heading, the shortcut above the ephemeral result, and necessary status/privacy feedback. Browser preview explores the two layouts honestly with disabled native controls; no IPC calls, microphone, fake ready status or fabricated results.

## Canonical components

- `ShortcutKeys` owns key formatting (Control→Ctrl, Space→Espace) and visual `kbd` elements. It is display-only; capture starts only through the real global shortcut.
- `SettingsPanel` owns native selects, text fields, checkboxes and the persisted settings form. Shared `.button` classes own visual button variants; labels always name their action. No custom select popup or overlay.
- `DictationPanel` owns status and read-only result. Result textarea has six rows, internal scrolling and `resize-none`; no manual resizing or layout jump. Delivery copy distinguishes inserted, copied, preview and unconfirmed insertion. The insertion_uncertain flag retains the possible partial-insertion warning after copying, even if another error code replaces the insertion error. Copier sends no text to IPC: Rust uses its current unexpired result. Effacer clears the backend result. Clipboard clear targets only the application-owned clipboard instance.
- `src/shared/runtime/useVoiceFlow.ts` owns native subscriptions, monotonic snapshot acceptance, settings loading and commands. Subscribe before initial status reads, release listeners on unmount, ignore late responses. No raw exceptions in UI or transcript logging.

## Settings behavior

Restore values from backend settings. Provider, local model/path, microphone, language, shortcut, automatic insertion and clipboard-copy consent live only in Réglages. Enregistrer et préparer first saves validated settings, then prepares the configured engine. Preserve edits on error and disable changes during active work. Changing a draft selection does not change the active engine. Groq reveals an audio-transfer explanation; no API-key input or secret appears in IPC. Local model choices remain preserved across provider toggles.

Automatic insertion starts enabled; automatic clipboard fallback consent starts disabled. Explain the X11 same-window caret limitation and permit disabling insertion. Duration remains a documented fixed60s ceiling; no nonfunctional duration control. Corrupt settings show an actionable state without pretending default values were successfully restored; saving is the explicit action that replaces the invalid file with displayed values.

## Icons, motion and accessibility

Iconify Lucide icons are imported as bundled data and rendered through `@iconify/react/offline`; no icon API/CDN calls. Decorative icons are `aria-hidden`; every icon button retains a text label. MicrophoneActivity distinguishes shortcut acknowledgement, backend-confirmed recording and processing. Green breathing rings appear only during native recording; they express activity, not measured audio volume. Processing uses a loader. Browser preview stays inactive. Reduced motion removes rings and rotation. Reduced motion and forced colors are covered in `src/styles.css`. Scrollbar colors share the existing palette; focus outlines use primary.

Canonical interaction ownership and checks are maintained in `UX-CONTRACT.md` and `premium-ui.json`.
