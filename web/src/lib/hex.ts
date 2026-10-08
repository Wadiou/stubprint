/** Bytes as lines of an offset, sixteen bytes in hexadecimal, and the same bytes as characters. */
export function hexDump(bytes: Uint8Array): string {
  const lines: string[] = [];
  for (let at = 0; at < bytes.length; at += 16) {
    const row = bytes.subarray(at, at + 16);
    const hex = Array.from(row, (byte) => byte.toString(16).padStart(2, "0")).join(" ");
    const text = Array.from(row, (byte) => (byte >= 0x20 && byte < 0x7f ? String.fromCharCode(byte) : ".")).join("");
    lines.push(`${at.toString(16).padStart(8, "0")}  ${hex.padEnd(47)}  ${text}`);
  }
  return lines.join("\n");
}
