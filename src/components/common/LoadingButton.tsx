import React from 'react';

interface LoadingButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  loading?: boolean;
  disabledReason?: string;
}

export function LoadingButton({ loading, disabledReason, children, ...props }: LoadingButtonProps) {
  const isDisabled = loading || props.disabled || !!disabledReason;
  return (
    <button 
      {...props} 
      disabled={isDisabled} 
      title={disabledReason}
      style={{
        ...props.style,
        cursor: isDisabled ? 'not-allowed' : 'pointer',
        opacity: isDisabled ? 0.7 : 1,
      }}
    >
      {loading ? 'Yükleniyor...' : children}
    </button>
  );
}
