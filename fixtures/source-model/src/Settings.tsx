import { useState } from "react";
import { Link } from "react-router";

export function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-2">
      <span>{label}</span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}

export function Settings() {
  const [notifications, setNotifications] = useState(true);
  return (
    <main className="flex flex-col gap-4 p-6">
      <h1 className="text-2xl font-semibold">Settings</h1>
      <Toggle label="Notifications" checked={notifications} onChange={setNotifications} />
      {notifications && <p className="text-muted">You will get an email for each order.</p>}
      <Link to="/">Back to orders</Link>
    </main>
  );
}
