object FormCroppingItem: TFormCroppingItem
  Left = 606
  Top = 367
  BorderStyle = bsDialog
  BorderWidth = 10
  ClientHeight = 364
  ClientWidth = 312
  Color = clBtnFace
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  OldCreateOrder = False
  OnCreate = FormCreate
  PixelsPerInch = 96
  TextHeight = 13
  object LabelFileName: TLabel
    Left = 24
    Top = 0
    Width = 70
    Height = 13
    Caption = 'LabelFileName'
  end
  object LabelSpeed: TLabel
    Left = 0
    Top = 118
    Width = 31
    Height = 13
    Caption = 'Speed'
  end
  object LabelDuration: TLabel
    Left = 24
    Top = 16
    Width = 40
    Height = 13
    Caption = 'Duration'
  end
  object LabelStart: TLabel
    Left = 24
    Top = 44
    Width = 22
    Height = 13
    Caption = 'Start'
  end
  object LabelFinish: TLabel
    Left = 24
    Top = 76
    Width = 27
    Height = 13
    Caption = 'Finish'
  end
  object LabelDurationValue: TLabel
    Left = 72
    Top = 16
    Width = 93
    Height = 13
    Caption = 'LabelDurationValue'
  end
  object LabelDateTime1: TLabel
    Left = 120
    Top = 44
    Width = 102
    Height = 13
    Caption = '14.04.2012  14:00:58'
  end
  object LabelDateTime2: TLabel
    Left = 120
    Top = 76
    Width = 102
    Height = 13
    Caption = '14.04.2012  14:01:17'
  end
  object ButtonOk: TButton
    Left = 32
    Top = 160
    Width = 75
    Height = 25
    Caption = 'Ok'
    Default = True
    TabOrder = 6
    OnClick = ButtonOkClick
  end
  object ButtonCancel: TButton
    Left = 128
    Top = 160
    Width = 75
    Height = 25
    Cancel = True
    Caption = 'Cancel'
    TabOrder = 7
    OnClick = ButtonCancelClick
  end
  object EditP1: TEdit
    Left = 248
    Top = 40
    Width = 65
    Height = 21
    TabOrder = 1
    Text = '0.000000'
    OnChange = EditP1Change
  end
  object EditP2: TEdit
    Left = 248
    Top = 72
    Width = 65
    Height = 21
    TabOrder = 3
    Text = '1.000000'
    OnChange = EditP2Change
  end
  object EditSpeed: TEdit
    Left = 264
    Top = 112
    Width = 49
    Height = 21
    TabOrder = 5
    Text = '1/16'
    OnChange = EditSpeedChange
  end
  object TrackBarSpeed: TTrackBar
    Left = 40
    Top = 104
    Width = 217
    Height = 33
    TabOrder = 4
    TabStop = False
    TickMarks = tmTopLeft
    OnChange = TrackBarSpeedChange
  end
  object EditTime1: TEdit
    Left = 56
    Top = 40
    Width = 57
    Height = 21
    TabOrder = 0
    Text = '0:00:00'
    OnChange = EditTime1Change
  end
  object EditTime2: TEdit
    Left = 56
    Top = 72
    Width = 57
    Height = 21
    TabOrder = 2
    Text = '1:23:45'
    OnChange = EditTime2Change
  end
  object SpeedUpMethod: TGroupBox
    Left = 16
    Top = 200
    Width = 241
    Height = 153
    Caption = 'Speed up method (no recompression are made)'
    TabOrder = 8
    object SUM_FPS_Caption: TLabel
      Left = 28
      Top = 16
      Width = 153
      Height = 65
      AutoSize = False
      Caption = 'Big FPS'
      WordWrap = True
      OnClick = SUM_FPS_CaptionClick
    end
    object SUM_KeyFrames_Caption: TLabel
      Left = 28
      Top = 81
      Width = 174
      Height = 65
      AutoSize = False
      Caption = 'Only key frames'
      WordWrap = True
      OnClick = SUM_KeyFrames_CaptionClick
    end
    object SUM_FPS: TRadioButton
      Left = 5
      Top = 16
      Width = 18
      Height = 18
      TabOrder = 0
      WordWrap = True
    end
    object SUM_KeyFrames: TRadioButton
      Left = 5
      Top = 81
      Width = 18
      Height = 18
      TabOrder = 1
      WordWrap = True
    end
  end
end
