import React from 'react';

interface LoadingButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  loading?: boolean;
  loadingText?: string;
  disabledReason?: string;
  projectWrite?: boolean;
}

export const LoadingButton = React.forwardRef<HTMLButtonElement, LoadingButtonProps>(
  function LoadingButton(
    {
      loading,
      loadingText,
      disabledReason,
      projectWrite = true,
      children,
      type,
      form,
      name,
      value,
      disabled,
      onClick,
      'aria-label': ariaLabel,
      'aria-disabled': ariaDisabled,
      ...rest
    },
    ref,
  ) {
    const isDisabled = loading || disabled || !!disabledReason;
    return (
      <button
        type={type}
        form={form}
        name={name}
        value={value}
        onClick={onClick}
        aria-label={ariaLabel}
        aria-disabled={ariaDisabled}
        {...rest}
        ref={ref}
        data-project-write={projectWrite ? 'true' : 'false'}
        disabled={isDisabled}
        title={disabledReason}
        style={{
          ...rest.style,
          cursor: isDisabled ? 'not-allowed' : 'pointer',
          opacity: isDisabled ? 0.7 : 1,
        }}
      >
        {loading ? (loadingText || 'Yükleniyor...') : children}
      </button>
    );
  },
);
