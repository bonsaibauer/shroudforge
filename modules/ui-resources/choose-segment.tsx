import type React from 'react'
import { useEffect, useRef, useState } from 'react'

export function Segmented({
  value,
  onChange,
  items,
}: {
  value: string
  onChange(v: string): void
  items: Array<[string, React.ReactNode]>
}) {
  const container = useRef<HTMLDivElement>(null)
  const [dividerOffsets, setDividerOffsets] = useState<number[]>([])

  useEffect(() => {
    const element = container.current
    if (!element) return

    const updateRows = () => {
      const rowTops = [...element.querySelectorAll<HTMLButtonElement>('button')]
        .map((button) => button.offsetTop)
        .filter((top, index, all) => all.indexOf(top) === index)
        .sort((left, right) => left - right)
      const nextOffsets = rowTops.slice(1).map((top) => top - 4.5)
      setDividerOffsets((current) =>
        current.length === nextOffsets.length &&
        current.every((offset, index) => offset === nextOffsets[index])
          ? current
          : nextOffsets,
      )
    }

    updateRows()
    const observer = new ResizeObserver(updateRows)
    observer.observe(element)
    element.querySelectorAll('button').forEach((button) => observer.observe(button))
    return () => observer.disconnect()
  }, [items])

  return (
    <div className="segments" ref={container}>
      {items.map(([id, label]) => (
        <button
          className={value === id ? 'active' : ''}
          key={id}
          onClick={() => onChange(id)}
        >
          {label}
        </button>
      ))}
      {dividerOffsets.map((top) => (
        <span
          aria-hidden="true"
          className="segments-row-divider"
          key={top}
          style={{ top }}
        />
      ))}
    </div>
  )
}
