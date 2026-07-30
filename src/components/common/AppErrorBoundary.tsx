import React from 'react';
import { Link } from 'react-router-dom';

type AppErrorBoundaryState = {
  hasError: boolean;
};

export type AppErrorBoundaryProps = React.PropsWithChildren;

export class AppErrorBoundary extends React.Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): AppErrorBoundaryState {
    return { hasError: true };
  }

  override componentDidCatch(error: unknown) {
    console.error('AppErrorBoundary caught an error', error);
  }

  override render() {
    if (this.state.hasError) {
      return (
        <div style={{ padding: '2rem' }}>
          <h1>Beklenmedik bir hata oluştu.</h1>
          <p>Sayfa güvenli biçimde kurtarıldı. Projeler sayfasına dönüp tekrar deneyin.</p>
          <Link to="/projects">Projects sayfasına git</Link>
        </div>
      );
    }

    return this.props.children;
  }
}
