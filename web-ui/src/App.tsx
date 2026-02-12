import { Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import Dashboard from './pages/Dashboard';
import Conflicts from './pages/Conflicts';
import ConflictDetail from './pages/ConflictDetail';
import Config from './pages/Config';
import AuditLog from './pages/AuditLog';
import Login from './pages/Login';

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const token = localStorage.getItem('session_token');
  if (!token) {
    return <Navigate to="/login" replace />;
  }
  return <>{children}</>;
}

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route
        path="/"
        element={
          <ProtectedRoute>
            <Layout />
          </ProtectedRoute>
        }
      >
        <Route index element={<Dashboard />} />
        <Route path="conflicts" element={<Conflicts />} />
        <Route path="conflicts/:id" element={<ConflictDetail />} />
        <Route path="config" element={<Config />} />
        <Route path="audit" element={<AuditLog />} />
      </Route>
    </Routes>
  );
}
