import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import App from "./App.jsx";
import { mockInvoke, mockListen } from "./test-setup.js";

/** Default happy path: tools installed, no app update pending. */
function readyApp(overrides = {}) {
  mockInvoke.mockImplementation((cmd, args) => {
    if (cmd in overrides) {
      const handler = overrides[cmd];
      return typeof handler === "function" ? handler(args) : Promise.resolve(handler);
    }
    switch (cmd) {
      case "tools_ready":
        return Promise.resolve(true);
      case "check_app_updates":
        return Promise.resolve({ available: false });
      case "ensure_setup":
        return Promise.resolve();
      default:
        return Promise.resolve();
    }
  });
}

beforeEach(() => {
  mockInvoke.mockReset();
  mockListen.mockReset();
  mockListen.mockImplementation(() => Promise.resolve(() => {}));
});

afterEach(cleanup);

describe("startup", () => {
  it("focuses the URL field so the user can paste straight away", async () => {
    readyApp();
    render(<App />);
    await waitFor(() => expect(screen.getByLabelText(/paste video link/i)).toHaveFocus());
  });

  it("defaults to the recommended format", async () => {
    readyApp();
    render(<App />);
    const best = await screen.findByRole("radio", { name: /best quality/i });
    expect(best).toBeChecked();
  });

  it("shows the privacy note from the spec", async () => {
    readyApp();
    render(<App />);
    // Await the settled state first: rendering kicks off async setup/update checks,
    // and asserting synchronously would race them (and log act() warnings).
    await screen.findByRole("button", { name: /^download$/i });
    expect(screen.getByText(/downloads stay on your computer/i)).toBeInTheDocument();
  });

  it("disables the form until setup has finished", async () => {
    readyApp({ tools_ready: false, ensure_setup: () => new Promise(() => {}) });
    render(<App />);
    await waitFor(() =>
      expect(screen.getByLabelText(/paste video link/i)).toBeDisabled()
    );
  });
});

describe("download", () => {
  it("keeps the button disabled until a link is entered", async () => {
    readyApp();
    render(<App />);
    const button = await screen.findByRole("button", { name: /^download$/i });
    expect(button).toBeDisabled();

    await userEvent.type(screen.getByLabelText(/paste video link/i), "https://youtu.be/x");
    expect(button).toBeEnabled();
  });

  it("passes the chosen format through to the backend", async () => {
    readyApp({ download_video: { file_path: "/d/song.mp3", file_name: "song.mp3", folder: "/d" } });
    render(<App />);

    await userEvent.type(
      await screen.findByLabelText(/paste video link/i),
      "https://youtu.be/x"
    );
    await userEvent.click(screen.getByRole("radio", { name: /audio only/i }));
    await userEvent.click(screen.getByRole("button", { name: /^download$/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("download_video", {
        url: "https://youtu.be/x",
        format: "audio_mp3",
      })
    );
  });

  it("disables the button while downloading", async () => {
    let release;
    readyApp({ download_video: () => new Promise((r) => (release = r)) });
    render(<App />);

    await userEvent.type(
      await screen.findByLabelText(/paste video link/i),
      "https://youtu.be/x"
    );
    await userEvent.click(screen.getByRole("button", { name: /^download$/i }));

    const button = await screen.findByRole("button", { name: /downloading/i });
    expect(button).toBeDisabled();

    // Let the download settle before the test ends, so the resulting state update
    // happens inside the test rather than against an unmounted tree.
    release({ file_path: "/d/v.mp4", file_name: "v.mp4", folder: "/d" });
    await screen.findByRole("alert");
  });

  it("announces success in the spec's words", async () => {
    readyApp({ download_video: { file_path: "/d/v.mp4", file_name: "v.mp4", folder: "/d" } });
    render(<App />);

    await userEvent.type(
      await screen.findByLabelText(/paste video link/i),
      "https://youtu.be/x"
    );
    await userEvent.click(screen.getByRole("button", { name: /^download$/i }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/your video is ready in downloads/i);
  });

  it("clears the success message after five seconds", async () => {
    // `shouldAdvanceTime` is load-bearing. Testing Library's findBy* polls on a real
    // timer; with plain fake timers that poll never fires and the test deadlocks
    // rather than failing. This keeps the clock ticking while still letting us jump
    // forward past the 5s window.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    try {
      readyApp({ download_video: { file_path: "/d/v.mp4", file_name: "v.mp4", folder: "/d" } });
      render(<App />);

      await user.type(await screen.findByLabelText(/paste video link/i), "https://youtu.be/x");
      await user.click(screen.getByRole("button", { name: /^download$/i }));
      expect(await screen.findByRole("alert")).toBeInTheDocument();

      await vi.advanceTimersByTimeAsync(5100);
      await waitFor(() =>
        expect(screen.queryByText(/is ready in downloads/i)).not.toBeInTheDocument()
      );
    } finally {
      // Restore even if the assertions throw, or every later test inherits fake timers.
      vi.useRealTimers();
    }
  });
});

describe("errors", () => {
  const failure = {
    message: "This video can't be downloaded right now. Try another?",
    detail: "ERROR: something technical",
    retryable: true,
  };

  async function triggerFailure() {
    readyApp({ download_video: () => Promise.reject(failure) });
    render(<App />);
    await userEvent.type(
      await screen.findByLabelText(/paste video link/i),
      "https://youtu.be/x"
    );
    await userEvent.click(screen.getByRole("button", { name: /^download$/i }));
  }

  it("shows the friendly message, not the technical one", async () => {
    await triggerFailure();
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/can't be downloaded right now/i);
    expect(alert).not.toHaveTextContent(/something technical/i);
  });

  it("offers a Try again button, per spec", async () => {
    await triggerFailure();
    expect(await screen.findByRole("button", { name: /try again/i })).toBeInTheDocument();
  });

  it("hides technical detail behind a disclosure", async () => {
    await triggerFailure();
    const toggle = await screen.findByRole("button", { name: /show technical details/i });
    expect(screen.queryByText(/something technical/i)).not.toBeInTheDocument();

    await userEvent.click(toggle);
    expect(await screen.findByText(/something technical/i)).toBeInTheDocument();
  });

  it("omits Try again when retrying cannot possibly help", async () => {
    readyApp({
      download_video: () =>
        Promise.reject({ message: "That video is private, so it can't be downloaded.", retryable: false }),
    });
    render(<App />);
    await userEvent.type(
      await screen.findByLabelText(/paste video link/i),
      "https://youtu.be/x"
    );
    await userEvent.click(screen.getByRole("button", { name: /^download$/i }));

    await screen.findByRole("alert");
    expect(screen.queryByRole("button", { name: /try again/i })).not.toBeInTheDocument();
  });

  it("re-enables the form after a failure so the user is not stuck", async () => {
    await triggerFailure();
    await screen.findByRole("alert");
    expect(screen.getByRole("button", { name: /^download$/i })).toBeEnabled();
  });
});

describe("app updates", () => {
  it("offers an update when one is available", async () => {
    readyApp({ check_app_updates: { available: true, version: "1.2.0" } });
    render(<App />);
    expect(await screen.findByText(/newer version/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /install now/i })).toBeInTheDocument();
  });

  it("can be dismissed with Later", async () => {
    readyApp({ check_app_updates: { available: true, version: "1.2.0" } });
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: /later/i }));
    expect(screen.queryByText(/newer version/i)).not.toBeInTheDocument();
  });

  it("stays silent when the update check fails", async () => {
    // Our server being down is not the user's problem and must not be shown to them.
    readyApp({ check_app_updates: () => Promise.reject(new Error("offline")) });
    render(<App />);
    await screen.findByRole("button", { name: /^download$/i });
    expect(screen.queryByText(/newer version/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("keyboard navigation", () => {
  it("reaches every control with Tab alone", async () => {
    readyApp();
    render(<App />);
    const input = await screen.findByLabelText(/paste video link/i);
    await userEvent.type(input, "https://youtu.be/x");

    await userEvent.tab();
    expect(screen.getByRole("radio", { name: /best quality/i })).toHaveFocus();

    // Radio groups are a single tab stop; arrows move within them.
    await userEvent.tab();
    expect(screen.getByRole("button", { name: /^download$/i })).toHaveFocus();
  });

  it("submits on Enter from the URL field", async () => {
    readyApp({ download_video: { file_path: "/d/v.mp4", file_name: "v.mp4", folder: "/d" } });
    render(<App />);
    await userEvent.type(
      await screen.findByLabelText(/paste video link/i),
      "https://youtu.be/x{Enter}"
    );
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("download_video", expect.anything())
    );
  });
});
