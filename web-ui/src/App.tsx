import { useState, useEffect } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import Dashboard from './pages/Dashboard';
import Conflicts from './pages/Conflicts';
import ConflictDetail from './pages/ConflictDetail';
import Config from './pages/Config';
import AuditLog from './pages/AuditLog';
import Login from './pages/Login';
import SetupWizard from './pages/SetupWizard';
import UserSettings from './pages/UserSettings';
import AdminUsers from './pages/AdminUsers';
import AdminLdap from './pages/AdminLdap';
import Repositories from './pages/Repositories';
import RepoDetail from './pages/RepoDetail';

// ---------------------------------------------------------------------------
// Splash Screen (React-controlled with minimum display time)
// ---------------------------------------------------------------------------

function SplashScreen() {
  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-gray-900"
         style={{ background: 'radial-gradient(ellipse at center, #1a2332 0%, #111827 70%)' }}>
      <div className="flex items-center gap-4 animate-pulse">
        <svg className="w-14 h-14" viewBox="0 0 56 56" fill="none" xmlns="http://www.w3.org/2000/svg"
             style={{ filter: 'drop-shadow(0 0 12px rgba(59, 130, 246, 0.4))' }}>
          <defs>
            <linearGradient id="splashGrad" x1="0" y1="0" x2="56" y2="56" gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="#3b82f6"/>
              <stop offset="100%" stopColor="#818cf8"/>
            </linearGradient>
            <linearGradient id="splashArrow" x1="10" y1="10" x2="46" y2="46" gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="#60a5fa"/>
              <stop offset="100%" stopColor="#a78bfa"/>
            </linearGradient>
          </defs>
          <circle cx="28" cy="28" r="25" stroke="url(#splashGrad)" strokeWidth="2" fill="none" opacity="0.3"/>
          <path d="M 38 14 A 16 16 0 0 1 42 28" stroke="url(#splashArrow)" strokeWidth="2.5" strokeLinecap="round" fill="none"/>
          <polygon points="43,27 42,31 39,28" fill="#60a5fa"/>
          <path d="M 18 42 A 16 16 0 0 1 14 28" stroke="url(#splashArrow)" strokeWidth="2.5" strokeLinecap="round" fill="none"/>
          <polygon points="13,29 14,25 17,28" fill="#a78bfa"/>
          <circle cx="28" cy="28" r="5" fill="#1e293b" stroke="url(#splashGrad)" strokeWidth="2"/>
          <circle cx="28" cy="28" r="2" fill="#60a5fa"/>
          <line x1="28" y1="23" x2="28" y2="14" stroke="#3b82f6" strokeWidth="1.5" strokeLinecap="round" opacity="0.7"/>
          <line x1="28" y1="33" x2="28" y2="42" stroke="#818cf8" strokeWidth="1.5" strokeLinecap="round" opacity="0.7"/>
          <circle cx="28" cy="13" r="2" fill="#3b82f6"/>
          <circle cx="28" cy="43" r="2" fill="#818cf8"/>
        </svg>
        <span className="text-4xl font-bold text-gray-100 font-display tracking-wider">
          RepoSync
        </span>
      </div>
      <span className="mt-5 text-xs font-display font-medium text-gray-500 tracking-[0.2em] uppercase">
        Repository Synchronization Platform
      </span>
      <div className="mt-6 w-7 h-7 border-3 border-gray-800 border-t-blue-500 border-r-indigo-400 rounded-full animate-spin" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const token = localStorage.getItem('session_token');
  if (!token) {
    return <Navigate to="/login" replace />;
  }
  return <>{children}</>;
}

export default function App() {
  // Only show splash on first visit per browser session — not on 401 redirects
  const alreadyShown = sessionStorage.getItem('splash_shown') === '1';
  const [showSplash, setShowSplash] = useState(!alreadyShown);
  const [fadeOut, setFadeOut] = useState(false);

  useEffect(() => {
    if (!showSplash) return;
    sessionStorage.setItem('splash_shown', '1');
    const timer = setTimeout(() => {
      setFadeOut(true);
      setTimeout(() => setShowSplash(false), 400);
    }, 2000);
    return () => clearTimeout(timer);
  }, [showSplash]);

  return (
    <>
      {showSplash && (
        <div
          className="transition-opacity duration-400"
          style={{ opacity: fadeOut ? 0 : 1, transition: 'opacity 0.4s ease-out' }}
        >
          <SplashScreen />
        </div>
      )}
      <div style={{ opacity: showSplash ? 0 : 1, transition: 'opacity 0.3s ease-in' }}>
        <Routes>
          <Route path="/login" element={<Login />} />
          <Route path="/setup" element={<SetupWizard />} />
          <Route
            path="/"
            element={
              <ProtectedRoute>
                <Layout />
              </ProtectedRoute>
            }
          >
            <Route index element={<Dashboard />} />
            <Route path="repos" element={<Repositories />} />
            <Route path="repos/:id" element={<RepoDetail />} />
            <Route path="conflicts" element={<Conflicts />} />
            <Route path="conflicts/:id" element={<ConflictDetail />} />
            <Route path="config" element={<Config />} />
            <Route path="audit" element={<AuditLog />} />
            <Route path="settings" element={<UserSettings />} />
            <Route path="users" element={<AdminUsers />} />
            <Route path="ldap" element={<AdminLdap />} />
          </Route>
        </Routes>
      </div>
    </>
  );
}
