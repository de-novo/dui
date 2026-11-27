// The empty archive state is also the entry to local source models.
export function EmptyWorkspace({ title, openModel }: { title: string; openModel: string }) {
  return (
    <section className="empty-workspace">
      <h2>{title}</h2>
      <code>dui map .</code>
      <a href="?view=model">{openModel}</a>
    </section>
  );
}
