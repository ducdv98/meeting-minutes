import { useState, type ReactNode } from "react";

export function MeetingRowMotion({ animate, children }: { animate: boolean; children: ReactNode }) {
  const [animateOnMount] = useState(animate);
  return <div className={animateOnMount ? "motion-meeting-enter" : undefined}>{children}</div>;
}
