import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

/**
 * The Tauri IPC bridge only exists inside a real webview, so the tests stub it.
 * Individual tests override `invoke` via `mockTauri()` below.
 */
export const mockInvoke = vi.fn();
export const mockListen = vi.fn(() => Promise.resolve(() => {}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args) => mockInvoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...args) => mockListen(...args) }));
