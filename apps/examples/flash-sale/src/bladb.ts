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

export interface RuntimeStage {
  role: string;
  action: string;
  cluster?: string;
}

export interface FlashSaleIdentity {
  app: string;
  uid: string;
  tenantId: string;
  displayName: string;
  email: string;
  roles: string[];
  anonymous: boolean;
  sessionKind: "authenticated" | "anonymous";
}

export interface FlashSaleSummary {
  identity: FlashSaleIdentity;
  item: SaleItemSummary;
  stock: number;
  wallet: number;
  orders: OrderRecord[];
  runtime: {
    readPath: RuntimeStage[];
    writePath: RuntimeStage[];
  };
}

export interface TicketStep {
  role: string;
  action: string;
  detail: string;
  at: string;
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
  steps: TicketStep[];
  runtime: {
    queueCluster: string;
    redisCluster: string;
    dbCluster: string;
  };
}

const flashSaleRoutes = {
  summary: appGet<FlashSaleSummary>("summary"),
  queuePurchase: appPost<{ sku: string; quantity: number }, QueueTicket>("queue"),
  queueHistory: appGet<QueueTicket[]>("queue"),
  queueTicket: appGet<[string], QueueTicket>((ticketId) => `queue/${ticketId}`)
};

export const flashSaleModule = createBrowserAppModule({
  baseUrl: BLADB_URL,
  appName: "flash-sale",
  tokenKey: FLASH_SALE_TOKEN_KEY,
  sessionKey: FLASH_SALE_SESSION_KEY,
  routes: flashSaleRoutes
});

export const db = flashSaleModule.db;
export const flashSaleUser = flashSaleModule.user;
export const flashSaleAuth = flashSaleModule.auth;
export const flashSaleApi = flashSaleModule.api;
