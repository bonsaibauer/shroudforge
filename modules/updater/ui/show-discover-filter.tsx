type DiscoverFilterBase = {
  name: string
  label: string
  options: Array<{ value: string; label: string }>
  disabled?: boolean
  className?: string
  summary?: 'label' | 'value' | 'both'
}

type SingleDiscoverFilter = DiscoverFilterBase & {
  multiple?: false
  value: string
  onChange: (value: string) => void
}

type MultipleDiscoverFilter = DiscoverFilterBase & {
  multiple: true
  value: string[]
  allValue: string
  onChange: (value: string[]) => void
}

export function DiscoverFilter(props: SingleDiscoverFilter | MultipleDiscoverFilter) {
  const {
    name,
    label,
    options,
    disabled = false,
    className,
    summary = 'both',
  } = props
  const selectedLabel = props.multiple
    ? props.value.length
      ? options.filter((option) => props.value.includes(option.value)).map((option) => option.label).join(', ')
      : options.find((option) => option.value === props.allValue)?.label || ''
    : options.find((option) => option.value === props.value)?.label || props.value
  const selection = props.multiple ? (
    <div role="group" aria-label={label}>
      {options.map((option) => {
        const isAll = option.value === props.allValue
        const checked = isAll ? props.value.length === 0 : props.value.includes(option.value)
        return (
          <label key={option.value}>
            <input
              type="checkbox"
              name={name}
              value={option.value}
              checked={checked}
              disabled={disabled}
              onChange={() => {
                if (isAll) {
                  props.onChange([])
                } else if (checked) {
                  props.onChange(props.value.filter((value) => value !== option.value))
                } else {
                  props.onChange([...props.value, option.value])
                }
              }}
            />
            {option.label}
          </label>
        )
      })}
    </div>
  ) : (
    <div role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <label key={option.value}>
          <input
            type="radio"
            name={name}
            value={option.value}
            checked={props.value === option.value}
            disabled={disabled}
            onChange={() => props.onChange(option.value)}
          />
          {option.label}
        </label>
      ))}
    </div>
  )

  return (
    <details className={`discover-option-filter${className ? ` ${className}` : ''}`}>
      <summary aria-disabled={disabled} aria-label={summary === 'value' ? label : undefined}>
        {summary === 'both' ? (
          <><span>{label}</span><strong>{selectedLabel}</strong></>
        ) : (
          <strong>{summary === 'label' ? label : selectedLabel}</strong>
        )}
      </summary>
      {selection}
    </details>
  )
}
