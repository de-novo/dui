<script>
  import "../../../../screen.css";
  let { data } = $props();
</script>

<svelte:head><title>{data.page.heading}</title></svelte:head>
<main>
  <h1>{data.page.heading}</h1>
  <p>{data.page.body}</p>
  <a href={data.next.path}>{data.next.heading}</a>
</main>
