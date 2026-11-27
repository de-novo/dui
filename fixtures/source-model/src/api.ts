export interface Order {
  id: string;
  title: string;
  status: "open" | "shipped";
  total?: number;
}

export declare function useOrders(): Order[];
