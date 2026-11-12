import { zNotice } from "./generated/zod.gen";

/** What the text of a live event says, checked against the description of the server, or nothing when it is not one of its notices. */
export function readNotice(text: string) {
  try {
    const read = zNotice.safeParse(JSON.parse(text));
    return read.success ? read.data : undefined;
  } catch {
    // The text was not even JSON.
    return undefined;
  }
}
