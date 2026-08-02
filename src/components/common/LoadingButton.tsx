import React from 'react';

interface LoadingButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  loading?: boolean;
  disabledReason?: string;
  projectWrite?: boolean;
}

export function LoadingButton({ loading, disabledReason, projectWrite = true, children, ...props }: LoadingButtonProps) {
  const isDisabled = loading || props.disabled || !!disabledReason;
  return (
    <button 
      {...props} 
      data-project-write={projectWrite ? 'true' : 'false'}
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
