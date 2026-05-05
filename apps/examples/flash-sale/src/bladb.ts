import {
  appGet,
  appPost,
  createBrowserAppModule,
  type GatewaySession
} from "@bladb/client";

export const BLADB_URL = import.meta.env.VITE_BLADB_URL ?? "http://localhost:8787";
export const FLASH_SALE_TOKEN_KEY = "bladb.flash-sale.token";
export const FLASH_SALE_SESSION_KEY = "bladb.flash-sale.session";

export type FlashSaleSession = GatewaySession;

export interface SaleItemSummary {
  id: string;
  sku: string;
  title: string;
  price: number;
  startsAt: string;
}

export interface OrderRecord {
  id: string;
  status: string;
  quantity: number;
  createdAt: string;
}

export interface FlashSaleSummary {
  item: SaleItemSummary;
  stock: number;
  wallet: number;
  orders: OrderRecord[];
}

export interface QueueTicket {
  ticketId: string;
  sku: string;
  quantity: number;
  status: "queued" | "processing" | "completed" | "failed";
  queuePosition: number | null;
  orderId: string | null;
  message: string;
  createdAt: string;
  updatedAt: string;
}

export const flashSaleModule = createBrowserAppModule({
  baseUrl: BLADB_URL,
  appName: "flash-sale",
  tokenKey: FLASH_SALE_TOKEN_KEY,
  sessionKey: FLASH_SALE_SESSION_KEY,
  routes: {
    summary: appGet<FlashSaleSummary>("summary"),
    queuePurchase: appPost<{ sku: string; quantity: number }, QueueTicket>("queue"),
    queueHistory: appGet<QueueTicket[]>("queue"),
    queueTicket: appGet<[string], QueueTicket>((ticketId) => `queue/${ticketId}`)
  }
});

export const db = flashSaleModule.db;
export const flashSaleUser = flashSaleModule.user;
export const flashSaleAuth = flashSaleModule.auth;
export const flashSaleApi = flashSaleModule.api;
