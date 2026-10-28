object FormGroup: TFormGroup
  Left = 341
  Top = 216
  Width = 713
  Height = 621
  BorderIcons = [biSystemMenu, biMaximize]
  Caption = 'Grouping'
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  KeyPreview = True
  OldCreateOrder = False
  OnCreate = FormCreate
  OnKeyDown = FormKeyDown
  OnResize = FormResize
  OnShow = FormShow
  PixelsPerInch = 96
  TextHeight = 13
  object Panel1: TPanel
    Left = 0
    Top = 542
    Width = 697
    Height = 41
    Align = alBottom
    BevelOuter = bvNone
    TabOrder = 1
    object ButtonGroup: TButton
      Left = 8
      Top = 8
      Width = 113
      Height = 25
      Caption = 'Group'
      TabOrder = 0
      OnClick = ButtonGroupClick
    end
    object ButtonClose: TButton
      Left = 560
      Top = 8
      Width = 75
      Height = 25
      Cancel = True
      Caption = 'Cancel'
      TabOrder = 1
      OnClick = ButtonCloseClick
    end
  end
  object ListView: TListView
    Left = 0
    Top = 0
    Width = 697
    Height = 542
    Align = alClient
    Checkboxes = True
    Columns = <>
    ColumnClick = False
    GridLines = True
    MultiSelect = True
    ReadOnly = True
    RowSelect = True
    TabOrder = 0
    ViewStyle = vsReport
    OnClick = ListViewClick
    OnSelectItem = ListViewSelectItem
  end
  object TimerDrawRoute: TTimer
    Interval = 500
    OnTimer = TimerDrawRouteTimer
    Left = 136
    Top = 272
  end
end
