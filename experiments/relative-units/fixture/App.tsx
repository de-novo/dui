export function App() {
  return (
    <main>
      <section>
        <div className="rem" />
      </section>
      <section className="parent">
        <div className="nested">
          <div className="em" />
        </div>
      </section>
      <section className="container">
        <div className="percent" />
      </section>
      <section className="container">
        <div className="padding">
          <div className="marker" />
        </div>
      </section>
      <section>
        <div className="viewport" />
      </section>
      <section>
        <div className="fluid" />
      </section>
      <section>
        <div className="inherited" />
      </section>
      <section className="container">
        <div style={{ padding: "5% 10%" }}>
          <div className="marker" />
        </div>
      </section>
    </main>
  );
}
