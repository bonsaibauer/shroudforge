import shroudEditHammer from '../../assets/shroudedit-hammer.png'

export function BrandMark() {
  return (
    <span className="brand-mark">
      <img src={shroudEditHammer} alt="" />
    </span>
  )
}

export function Brand() {
  return (
    <div className="brand">
      <BrandMark />
      <span>
        <strong>SHROUDFORGE</strong>
        <small>MODLOADER</small>
      </span>
    </div>
  )
}
