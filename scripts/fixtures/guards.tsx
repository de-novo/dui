// Exercise the same narrowing, nested arms and short-circuiting as real UI code.
export function Guards({ item }: { item: { title: string } | null }) {
  if (item)
    if (item.title === "") return <i>Untitled</i>;
    else return <i>{item.title}</i>;
  return <i>Missing</i>;
}

export function Inline({ item }: { item?: { title: string } }) {
  return (
    <>
      {item ? item.title : "Missing"}
      {item && <i>{item.title}</i>}
      {item?.title || <i>Untitled</i>}
    </>
  );
}
