import { z } from "zod";

// What the address of a page can carry. A value that does not fit is dropped, so a hand typed
// address never breaks the page.

export const liveSearch = z.object({ receipt: z.number().optional() }).catch({});

export const printersSearch = z
  .object({
    printer: z.string().optional(),
    // The address can say `add` or `add=true`.
    add: z
      .union([z.literal(true), z.literal("true")])
      .transform(() => true)
      .optional(),
    remove: z.string().optional(),
  })
  .catch({});
