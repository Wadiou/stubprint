import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { getReceiptBytesOptions } from "@/api/generated/@tanstack/react-query.gen";

/** The bytes a receipt was made of, once they are read. */
export function useReceiptBytes(id: number) {
  const file = useQuery({ ...getReceiptBytesOptions({ path: { id } }), staleTime: Number.POSITIVE_INFINITY });
  const [bytes, setBytes] = useState<Uint8Array>();
  useEffect(() => {
    let current = true;
    file.data?.arrayBuffer().then((buffer) => current && setBytes(new Uint8Array(buffer)));
    return () => {
      current = false;
    };
  }, [file.data]);
  return bytes;
}
