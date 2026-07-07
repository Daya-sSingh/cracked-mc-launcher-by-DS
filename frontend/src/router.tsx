import { lazy, Suspense } from "react";
import { createBrowserRouter, RouterProvider, Navigate } from "react-router-dom";
import { MainLayout } from "@/components/layout/MainLayout";
import { Spinner } from "@/components/shared/Feedback";

// Lazy-load pages so the initial bundle stays small — only Home and the
// shared layout shell load eagerly; Library/Downloads/Settings load the
// first time you click their sidebar item.
const HomePage      = lazy(() => import("@/pages/Home"));
const LibraryPage   = lazy(() => import("@/pages/Library"));
const DownloadsPage = lazy(() => import("@/pages/Downloads"));
const SettingsPage  = lazy(() => import("@/pages/Settings"));
const InstanceDetailPage   = lazy(() => import("@/pages/InstanceDetail"));
const InstanceSettingsPage = lazy(() => import("@/pages/InstanceSettings"));

function PageFallback() {
  return (
    <div className="flex h-full items-center justify-center">
      <Spinner size={24} />
    </div>
  );
}

const router = createBrowserRouter([
  {
    path: "/",
    element: <MainLayout />,
    children: [
      { index: true, element: (
        <Suspense fallback={<PageFallback />}><HomePage /></Suspense>
      )},
      { path: "library", element: (
        <Suspense fallback={<PageFallback />}><LibraryPage /></Suspense>
      )},
      { path: "downloads", element: (
        <Suspense fallback={<PageFallback />}><DownloadsPage /></Suspense>
      )},
      { path: "settings", element: (
        <Suspense fallback={<PageFallback />}><SettingsPage /></Suspense>
      )},
      { path: "instance/:id", element: (
        <Suspense fallback={<PageFallback />}><InstanceDetailPage /></Suspense>
      )},
      { path: "instance/:id/settings", element: (
        <Suspense fallback={<PageFallback />}><InstanceSettingsPage /></Suspense>
      )},
      // Catch-all: send unknown paths back to home
      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
