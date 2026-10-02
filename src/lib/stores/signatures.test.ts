import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";

const tauri = vi.hoisted(() => ({ commitSignatures: vi.fn() }));
vi.mock("../tauri", () => tauri);

const { signatureFor, clearSignatureCache } = await import("./signatures");
const wait = () => new Promise((r) => setTimeout(r, 60));

beforeEach(() => {
  tauri.commitSignatures.mockReset();
  clearSignatureCache();
});

describe("signatureFor", () => {
  it("batches rows of one tick into one call and caches results", async () => {
    tauri.commitSignatures.mockResolvedValue([{ oid: "a", status: "good", signer: "Ada", key: "K" }]);
    const a = signatureFor("/r", "a");
    const b = signatureFor("/r", "b");
    expect(get(a)).toBeNull();
    await wait();
    expect(tauri.commitSignatures).toHaveBeenCalledTimes(1);
    expect(tauri.commitSignatures).toHaveBeenCalledWith("/r", ["a", "b"]);
    expect(get(a)?.status).toBe("good");
    expect(get(b)).toBeNull(); // unsigned
    signatureFor("/r", "a");
    signatureFor("/r", "b");
    await wait();
    expect(tauri.commitSignatures).toHaveBeenCalledTimes(1); // cached, incl. unsigned
  });

  it("keeps repositories apart and survives backend errors", async () => {
    tauri.commitSignatures.mockRejectedValueOnce(new Error("no git"));
    tauri.commitSignatures.mockResolvedValueOnce([{ oid: "a", status: "bad", signer: null, key: null }]);
    const one = signatureFor("/one", "a");
    const two = signatureFor("/two", "a");
    await wait();
    expect(tauri.commitSignatures).toHaveBeenCalledTimes(2);
    expect(get(one)).toBeNull();
    expect(get(two)?.status).toBe("bad");
  });

  it("does nothing without a repository", async () => {
    expect(get(signatureFor(null, "a"))).toBeNull();
    await wait();
    expect(tauri.commitSignatures).not.toHaveBeenCalled();
  });
});
