object FormFileProperty: TFormFileProperty
  Left = 192
  Top = 124
  Width = 441
  Height = 494
  Caption = 'FormFileProperty'
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  Position = poDefaultPosOnly
  OnClose = FormClose
  OnCreate = FormCreate
  OnResize = FormResize
  PixelsPerInch = 96
  TextHeight = 13
  object StringGrid: TStringGrid
    Left = 0
    Top = 0
    Width = 425
    Height = 415
    Align = alClient
    BorderStyle = bsNone
    ColCount = 2
    Ctl3D = False
    DefaultColWidth = 100
    DefaultRowHeight = 16
    FixedRows = 0
    Options = [goFixedVertLine, goFixedHorzLine, goVertLine, goHorzLine, goRangeSelect, goEditing, goThumbTracking]
    ParentCtl3D = False
    ScrollBars = ssVertical
    TabOrder = 0
    RowHeights = (
      16
      16
      16
      16
      16
    )
  end
  object Panel1: TPanel
    Left = 0
    Top = 415
    Width = 425
    Height = 41
    Align = alBottom
    BevelOuter = bvNone
    TabOrder = 1
    object ButtonCopy: TButton
      Left = 8
      Top = 8
      Width = 217
      Height = 25
      Caption = 'Copy All'
      TabOrder = 0
      OnClick = ButtonCopyClick
    end
  end
end
