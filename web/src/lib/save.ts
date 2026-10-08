/** Hands a file that was fetched to the person, as a download. */
export function save(file: BlobPart, name: string) {
  const url = URL.createObjectURL(file instanceof Blob ? file : new Blob([file]));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}
