import { StrictMode } from 'react';
import { act, cleanup, render, renderHook, screen, fireEvent, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useClipboardEntries } from './useClipboardEntries';
import { useOverlayLifecycle } from './useOverlayLifecycle';
import { Overlay } from '../components/Overlay';
import type { Entry } from '../lib/types';

const mocks = vi.hoisted(() => ({ list: vi.fn(), paste: vi.fn(), hide: vi.fn(), listen: vi.fn(), focus: vi.fn() }));
vi.mock('../lib/api', () => ({ api: { ...mocks, togglePin: vi.fn(), delete: vi.fn() } }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ onFocusChanged: mocks.focus, hide: mocks.hide }) }));
const callbacks = new Map<string, Set<() => void>>();
const emit = (event: string) => callbacks.get(event)?.forEach(fn => fn());
const entry = (id: number, lastUsedAt = id): Entry => ({ id, lastUsedAt, createdAt: 0, text: `item ${id}`, kind: 'text', pinned: false, imagePath: null, thumbB64: null, width: null, height: null, sizeBytes: 6, contentHash: `${id}` });
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r; }); return { promise, resolve }; }
beforeEach(() => {
  vi.resetAllMocks(); callbacks.clear();
  Element.prototype.scrollIntoView = vi.fn();
  mocks.list.mockResolvedValue([entry(1)]);
  mocks.hide.mockResolvedValue(undefined);
  mocks.listen.mockImplementation(async (event: string, fn: () => void) => { const listeners = callbacks.get(event) ?? new Set<() => void>(); listeners.add(fn); callbacks.set(event, listeners); return () => { listeners.delete(fn); }; });
  mocks.focus.mockResolvedValue(() => {});
});
afterEach(cleanup);
it('refreshes on open, sorts ties, and ignores older responses', async () => {
  const { result } = renderHook(() => useClipboardEntries(''));
  await waitFor(() => expect(result.current.loading).toBe(false));
  const old = deferred<Entry[]>(); const recent = deferred<Entry[]>();
  mocks.list.mockReturnValueOnce(old.promise).mockReturnValueOnce(recent.promise);
  act(() => { emit('overlay:opened'); emit('clipboard:entries-changed'); });
  await act(async () => recent.resolve([entry(2, 10), entry(3, 10)]));
  await act(async () => old.resolve([entry(1)]));
  expect(result.current.filtered.recent.map(e => e.entry.id)).toEqual([3, 2]);
});
it('disposes late registrations and ignores old callbacks under StrictMode', async () => {
  const pending: Array<{ resolve: (value: () => void) => void; dispose: ReturnType<typeof vi.fn>; callback: () => void }> = [];
  mocks.listen.mockImplementation((_event: string, callback: () => void) => {
    const d = deferred<() => void>(); pending.push({ resolve: d.resolve, dispose: vi.fn(), callback }); return d.promise;
  });
  const { unmount } = renderHook(() => useClipboardEntries(''), { wrapper: StrictMode });
  expect(pending).toHaveLength(2);
  unmount();
  await act(async () => { pending.forEach(p => p.resolve(p.dispose)); });
  pending.forEach(p => { expect(p.dispose).toHaveBeenCalledOnce(); p.callback(); });
  expect(mocks.list).not.toHaveBeenCalled();
});
it('disposes a late native focus listener', async () => {
  const late = deferred<() => void>(); const dispose = vi.fn();
  mocks.focus.mockReturnValue(late.promise);
  const opened = vi.fn();
  const { unmount } = renderHook(() => useOverlayLifecycle({ onOpened: opened }));
  await waitFor(() => expect(mocks.focus).toHaveBeenCalled());
  const callback = mocks.focus.mock.calls[0][0];
  unmount();
  await act(async () => late.resolve(dispose));
  callback({ payload: true });
  expect(dispose).toHaveBeenCalledOnce(); expect(opened).not.toHaveBeenCalled();
});
it('handles registration rejection and still loads entries', async () => {
  const log = vi.spyOn(console, 'error').mockImplementation(() => {});
  mocks.listen.mockRejectedValueOnce(new Error('registration failed'));
  const { result } = renderHook(() => useClipboardEntries(''));
  await waitFor(() => expect(result.current.loading).toBe(false));
  expect(result.current.entries).toHaveLength(1); log.mockRestore();
});
it.each([true, false, 'error'])('closes only after successful copy: %s', async (outcome) => {
  const log = vi.spyOn(console, 'error').mockImplementation(() => {});
  if (outcome === 'error') mocks.paste.mockRejectedValue(new Error('write failed')); else mocks.paste.mockResolvedValue(outcome);
  render(<Overlay />);
  await screen.findByText('item 1');
  await act(async () => {});
  fireEvent.keyDown(window, { key: 'Enter' });
  await waitFor(() => expect(mocks.paste).toHaveBeenCalledWith(1));
  await act(async () => {});
  expect(mocks.hide).toHaveBeenCalledTimes(outcome === true ? 1 : 0); log.mockRestore();
});
it('ignores a list response after unmount', async () => {
  const pending = deferred<Entry[]>(); mocks.list.mockReturnValue(pending.promise);
  const { unmount, result } = renderHook(() => useClipboardEntries(''));
  await waitFor(() => expect(mocks.list).toHaveBeenCalledOnce());
  unmount();
  await act(async () => pending.resolve([entry(9)]));
  expect(result.current.entries).toEqual([]);
});
it('resets selection on reopen and does not select newly typed text on focus', async () => {
  mocks.list.mockResolvedValue([entry(1), entry(2)]); mocks.paste.mockResolvedValue(true);
  render(<Overlay />);
  await screen.findByText('item 2');
  await act(async () => {});
  fireEvent.keyDown(window, { key: 'ArrowDown' });
  act(() => emit('overlay:opened'));
  expect(screen.getAllByRole('option')[0].getAttribute('aria-selected')).toBe('true');
  const input = screen.getByRole('textbox') as HTMLInputElement;
  fireEvent.change(input, { target: { value: 'item' } });
  input.setSelectionRange(4, 4);
  act(() => mocks.focus.mock.calls[mocks.focus.mock.calls.length - 1][0]({ payload: true }));
  expect(input.selectionStart).toBe(4); expect(input.selectionEnd).toBe(4);
  fireEvent.keyDown(window, { key: 'Enter' });
  await waitFor(() => expect(mocks.paste).toHaveBeenCalledWith(1));
});
