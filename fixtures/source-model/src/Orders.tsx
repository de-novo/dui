import { useState } from "react";
import { Link } from "react-router";
import { useOrders } from "./api";
import styles from "./Orders.module.css";

export function Orders() {
  const orders = useOrders();
  const [filter, setFilter] = useState<"all" | "open">("all");
  return (
    <main className="flex flex-col gap-4 p-6 md:p-8">
      <h1 className="text-2xl font-semibold text-brand">Orders</h1>
      <button
        type="button"
        className="rounded-card bg-brand px-3 py-2 text-on-brand"
        onClick={() => setFilter(filter === "all" ? "open" : "all")}
      >
        {filter === "all" ? "Show open" : "Show all"}
      </button>
      {orders.length === 0 ? (
        <p className="text-muted">No orders yet</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {orders.map((order) => (
            <li key={order.id} style={{ padding: 12, borderRadius: 8 }}>
              <span>{order.title}</span>
              <span className={styles.status}>{order.status}</span>
            </li>
          ))}
        </ul>
      )}
      <Link to="/settings">Settings</Link>
    </main>
  );
}
