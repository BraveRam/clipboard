/** Own asynchronous native subscriptions even when registration outlives an effect. */
export function subscriptionScope() {
  let active = true;
  const disposers: Array<() => void> = [];
  return {
    get active() { return active; },
    async add(register: () => Promise<() => void>) {
      if (!active) return;
      try {
        const dispose = await register();
        if (active) disposers.push(dispose);
        else dispose();
      } catch (error) {
        if (active) console.error("register event listener", error);
      }
    },
    dispose() {
      active = false;
      disposers.splice(0).forEach((dispose) => dispose());
    },
  };
}
