import './Banner.css';

/** A problem the person should read, with a way to put it away. */
export function Banner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div className="banner" role="alert">
      <p>{message}</p>
      <button type="button" className="quiet" onClick={onDismiss}>
        Dismiss
      </button>
    </div>
  );
}
