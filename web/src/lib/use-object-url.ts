import { useEffect, useState } from "react";

/** An address for a file that was fetched, to show it in the page. It is let go when the file changes. */
export function useObjectUrl(file?: Blob) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    if (!file) return setUrl(undefined);
    const made = URL.createObjectURL(file);
    setUrl(made);
    return () => URL.revokeObjectURL(made);
  }, [file]);
  return url;
}
