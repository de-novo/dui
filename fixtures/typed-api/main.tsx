import { createRoot } from "react-dom/client";

type Book = { id: string; title: string; cover: string; price: number };
type Shelf = { books: Book[] };

async function fetchShelf(): Promise<Shelf> {
  const response = await fetch("/api/shelf");
  return response.json();
}

async function loadShelf() {
  const shelf = await fetchShelf();
  createRoot(document.getElementById("app")!).render(
    <main aria-label="Shelf">
      {shelf.books.map((book) => (
        <article key={book.id}>
          <img width={120} height={90} src={book.cover} alt={book.title} />
          <h2>{book.title}</h2>
          <span>{book.price}</span>
        </article>
      ))}
    </main>,
  );
}
void loadShelf();
